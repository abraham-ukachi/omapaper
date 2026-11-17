//! Agent workflow: whoever asks — a human, an @mention in a comment, or an
//! automation ticking — the work lands on a board card, and a card can be run
//! by the built-in agent runner when a key is configured.

use crate::api::{dispatch_webhooks, Shared};
use omapaper_core::agent::{self, AgentConfig, TaskContext};
use omapaper_core::assets;
use omapaper_core::model::*;
use omapaper_core::StoreError;
use serde_json::json;

/// The first `@role` in a comment body, if it names a specialist.
pub fn mention_role(body: &str) -> Option<CardRole> {
    body.split_whitespace()
        .find_map(|word| CardRole::parse(word.trim_end_matches([',', '.', ':', ';', '!', '?'])))
}

/// A card title from a comment: the first line, mentions removed and cut short.
pub fn title_from(body: &str) -> String {
    let line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Untitled task");
    let words: Vec<&str> = line
        .split_whitespace()
        .filter(|word| !word.starts_with('@'))
        .collect();
    let text = if words.is_empty() {
        line.to_string()
    } else {
        words.join(" ")
    };
    let title: String = text.chars().take(64).collect();
    if text.chars().count() > 64 {
        format!("{title}…")
    } else {
        title
    }
}

/// Queues a card, tells every open tab and fires `card.created` webhooks.
pub fn queue_card(
    app: &Shared,
    canvas_id: &str,
    title: &str,
    brief: &str,
    role: CardRole,
    frame_id: Option<String>,
    created_by: &str,
) -> Result<BoardCard, StoreError> {
    let mut card = BoardCard::new(canvas_id, title, brief, role);
    card.frame_id = frame_id;
    card.created_by = created_by.to_string();
    app.host.store.add_card(&card)?;
    app.host.store.touch_canvas(canvas_id)?;
    app.live.send(json!({
        "type": "card.created",
        "canvasId": card.canvas_id,
        "card": card,
    }));
    dispatch_webhooks(
        app.clone(),
        canvas_id.to_string(),
        "card.created",
        json!(card.clone()),
    );
    Ok(card)
}

fn save_and_announce(app: &Shared, card: &BoardCard) {
    let _ = app.host.store.save_card(card);
    app.live.send(json!({
        "type": "card.updated",
        "canvasId": card.canvas_id,
        "card": card,
    }));
}

/// Runs a card with the built-in runner. Safe to call without a key: the card
/// simply fails with the reason, which is what the board should show.
pub async fn run_card(app: Shared, card_id: String) {
    let Ok(Some(card)) = app.host.store.get_card(&card_id) else {
        return;
    };
    if card.status == CardStatus::Working {
        return;
    }
    let Some(config) = AgentConfig::from_env() else {
        let mut failed = card.clone();
        failed.status = CardStatus::Failed;
        failed.error = Some(
            "no agent key: set OMAPAPER_AGENT_API_KEY (or OPENAI_API_KEY) and restart omapaperd"
                .into(),
        );
        save_and_announce(&app, &failed);
        return;
    };

    let canvas = match app.host.store.get_canvas(&card.canvas_id) {
        Ok(Some(canvas)) => canvas,
        _ => return,
    };
    let frame = card
        .frame_id
        .as_deref()
        .and_then(|id| app.host.store.get_frame(id).ok().flatten());

    let mut working = card.clone();
    working.status = CardStatus::Working;
    working.error = None;
    save_and_announce(&app, &working);

    let system = agent::system_prompt(card.role);
    let prompt = agent::user_prompt(&TaskContext {
        card: &card,
        canvas: &canvas,
        frame: frame.as_ref(),
    });

    let reply = tokio::task::spawn_blocking(move || agent::run(&config, &system, &prompt)).await;

    let mut done = card.clone();
    let outcome = match reply {
        Ok(Ok(reply)) => match agent::extract_document(&reply) {
            Some(document) => {
                if let Some(mut frame) = frame.clone() {
                    let _ = app
                        .host
                        .store
                        .push_version(&frame, Some("before an agent task"), 30);
                    frame.html = document;
                    frame.revision += 1;
                    frame.approval = Approval::Draft;
                    frame.approval_by = None;
                    let _ = app.host.store.save_frame(&frame);
                    app.live.send(json!({
                        "type": "frame.updated",
                        "canvasId": frame.canvas_id,
                        "frame": frame,
                    }));
                    done.frame_id = Some(frame.id.clone());
                    format!("Rewrote {} (revision {})", frame.name, frame.revision)
                } else {
                    let page = canvas
                        .pages
                        .first()
                        .map(|page| page.id.clone())
                        .unwrap_or_default();
                    let mut created = Frame::new(&card.canvas_id, &page, &card.title);
                    created.html = document;
                    if app.host.store.insert_frame(&created).is_ok() {
                        app.live.send(json!({
                            "type": "frame.created",
                            "canvasId": created.canvas_id,
                            "frame": created,
                        }));
                        done.frame_id = Some(created.id.clone());
                        format!("Created frame {}", created.name)
                    } else {
                        "The agent answered, but the frame could not be created".to_string()
                    }
                }
            }
            None => {
                done.status = CardStatus::Failed;
                done.error = Some(format!(
                    "the reply was not an HTML document: {}",
                    reply.chars().take(180).collect::<String>()
                ));
                done.status.key().to_string()
            }
        },
        Ok(Err(error)) => {
            done.status = CardStatus::Failed;
            done.error = Some(error);
            "failed".into()
        }
        Err(error) => {
            done.status = CardStatus::Failed;
            done.error = Some(format!("the runner stopped: {error}"));
            "failed".into()
        }
    };

    if done.status != CardStatus::Failed {
        done.status = CardStatus::Done;
        done.result = Some(outcome);
    }
    save_and_announce(&app, &done);
    app.host.store.touch_canvas(&done.canvas_id).ok();
    let event = if done.status == CardStatus::Done {
        "agent.completed"
    } else {
        "agent.failed"
    };
    dispatch_webhooks(app.clone(), done.canvas_id.clone(), event, json!(done));
}

/// Fires every enabled automation for `trigger` on a canvas. `note` is added to
/// the card brief so the specialist knows what happened.
pub fn fire_automations(app: &Shared, canvas_id: &str, trigger: AutomationTrigger, note: &str) {
    let Ok(automations) = app.host.store.automations(canvas_id) else {
        return;
    };
    for mut automation in automations {
        if !automation.enabled || automation.trigger != trigger {
            continue;
        }
        let brief = if note.is_empty() {
            automation.brief.clone()
        } else {
            format!("{}\n\n{}", automation.brief, note)
        };
        let created_by = format!("automation:{}", automation.name);
        if queue_card(
            app,
            canvas_id,
            &automation.name,
            &brief,
            automation.role,
            None,
            &created_by,
        )
        .is_ok()
        {
            automation.last_run_at = Some(now_ms());
            let _ = app.host.store.save_automation(&automation);
        }
    }
}

/// One scheduler tick: queues the cards whose interval has come round.
pub fn scheduled_tick(app: &Shared) {
    let Ok(canvases) = app.host.store.list_canvases() else {
        return;
    };
    let now = now_ms();
    for canvas in canvases {
        let Ok(automations) = app.host.store.automations(&canvas.id) else {
            continue;
        };
        for mut automation in automations {
            if !automation.is_due(now) {
                continue;
            }
            if queue_card(
                app,
                &canvas.id,
                &automation.name,
                &automation.brief,
                automation.role,
                None,
                &format!("automation:{}", automation.name),
            )
            .is_ok()
            {
                automation.last_run_at = Some(now);
                let _ = app.host.store.save_automation(&automation);
            }
        }
    }
}

/// A placed SVG: the markup plus where it goes.
pub struct SvgLayer<'a> {
    pub svg: &'a str,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub name: Option<&'a str>,
}

/// Inserts a generated SVG into a frame as a positioned layer.
pub fn insert_svg(app: &Shared, frame_id: &str, layer: &SvgLayer<'_>) -> Result<Frame, String> {
    let mut frame = app
        .host
        .store
        .get_frame(frame_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no frame `{frame_id}`"))?;
    let layer = assets::svg_layer(
        layer.svg,
        layer.x,
        layer.y,
        layer.width,
        layer.height,
        layer.name,
    )?;
    let html = assets::insert_before_body(&frame.html, &layer);
    app.host
        .store
        .push_version(&frame, Some("before an SVG insert"), 30)
        .map_err(|error| error.to_string())?;
    frame.html = html;
    frame.revision += 1;
    app.host
        .store
        .save_frame(&frame)
        .map_err(|error| error.to_string())?;
    app.live.send(json!({
        "type": "frame.updated",
        "canvasId": frame.canvas_id,
        "frame": frame,
    }));
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_name_a_specialist() {
        assert_eq!(
            mention_role("hey @design tighten the hero"),
            Some(CardRole::Design)
        );
        assert_eq!(mention_role("@copy: shorter please"), Some(CardRole::Copy));
        assert_eq!(mention_role("@nobody here"), None);
        assert_eq!(mention_role("plain note"), None);
    }

    #[test]
    fn titles_are_short_and_mention_free() {
        assert_eq!(title_from("@design Tighten the hero"), "Tighten the hero");
        let long = title_from(&"word ".repeat(40));
        assert!(long.chars().count() <= 65, "{long}");
        assert_eq!(title_from("   "), "Untitled task");
    }
}
