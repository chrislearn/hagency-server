use super::common::*;
use dioxus::prelude::*;
use serde_json::{Value, json};

#[component]
pub fn RoomDiscovery(project_id: Signal<String>, room: Signal<String>) -> Element {
    let mut rooms = use_resource(move || {
        let project = project_id();
        async move {
            let value = if project.is_empty() {
                Ok(json!({"rooms":[]}))
            } else {
                browser_auth::call(
                    &format!("/projects/{}/rooms", urlencoding::encode(&project)),
                    "GET",
                    None,
                )
                .await
                .map_err(|e| e.message)
            };
            (project, value)
        }
    });
    let mut agents = use_resource(move || {
        let project = project_id();
        let selected = room();
        async move {
            let value = if project.is_empty() || selected.is_empty() {
                Ok(json!({"agents":[]}))
            } else {
                browser_auth::call(
                    &format!(
                        "/projects/{}/rooms/{}/agents",
                        urlencoding::encode(&project),
                        urlencoding::encode(&selected)
                    ),
                    "GET",
                    None,
                )
                .await
                .map_err(|e| e.message)
            };
            (project, selected, value)
        }
    });
    let data = rooms()
        .filter(|(project, _)| *project == project_id())
        .and_then(|(_, v)| v.ok())
        .unwrap_or(Value::Null);
    let roster = agents()
        .filter(|(project, selected, _)| *project == project_id() && *selected == room())
        .and_then(|(_, _, v)| v.ok())
        .unwrap_or(Value::Null);
    rsx! {section { class:"hg-card hg-stack",
        h2 { "My Rooms and their Agents" }
        p { "Only registered child Rooms you currently belong to are listed. Space membership does not grant private Room membership. A Room member may view its Agent roster without joining the Space." }
        button { class:"hg-button",disabled:project_id().is_empty(),onclick:move|_|{rooms.restart();agents.restart();},"Refresh Rooms and roster" }
        if let Some((project,Err(error)))=rooms() { if project==project_id() {p{role:"alert","{error}"}} }
        for item in rows(&data,"rooms") {div { class:"hg-stack",key:"{item}",
            code {{text(&item,"roomId")}}
            button {class:"hg-button",onclick:move|_|room.set(text(&item,"roomId")),"Select Room"}
        }}
        if !room().is_empty() {p{{format!("Selected Room: {}",room())}}}
        if let Some((project,selected,Err(error)))=agents() {if project==project_id()&&selected==room(){p{role:"alert","{error}"}}}
        for agent in rows(&roster,"agents") {div{class:"hg-stack",key:"{agent}",
            strong {{text(&agent,"displayName")}}
            code {{text(&agent,"puppetMxid")}}
            p {{format!("Creator: {}; binding: {}",text(&agent,"ownerMxid"),text(&agent,"bindingState"))}}
        }}
        p {class:"hg-note","Binding state is not proof that the creator’s device or model is currently online. Agent ownership is permanent; this view does not grant administration rights."}
    }}
}
