use super::common::*;
use dioxus::prelude::*;
use serde_json::{Value, json};
#[component]
pub fn Projects() -> Element {
    let mut projects = use_resource(|| async {
        browser_auth::call("/projects", "GET", None)
            .await
            .map_err(|e| e.message)
    });
    let mut message = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut space = use_signal(String::new);
    let mut project_id = use_signal(String::new);
    let mut room = use_signal(String::new);
    let mut revision = use_signal(|| String::from("1"));
    let mut default_allow = use_signal(|| true);
    let mut allow = use_signal(String::new);
    let mut deny = use_signal(String::new);
    let data = projects().and_then(Result::ok).unwrap_or(Value::Null);
    rsx! {div{class:"hg-page hg-stack",
        h1{class:"text-2xl font-bold","Projects"}
        p{"Each project maps to one Matrix Space. Rooms have independent membership. Space or Room administrators can register existing groups and manage Room access rights below."}
        Notice{message}
        button{class:"hg-button",onclick:move|_|projects.restart(),"Refresh"}
        if let Some(Err(error))=projects(){p{role:"alert","{error}"}}
        for project in rows(&data,"projects") {
            div{class:"hg-card hg-stack",key:"{project}",
                strong{{text(&project,"spaceId")}}
                code{{text(&project,"id")}}
                p{{format!("Revision {}; active {}",project["revision"],project["active"])}}
                p{class:"hg-note","Room access policy is managed below after selecting this project."}
                button{class:"hg-button",onclick:move|_|{
                    project_id.set(text(&project,"id"));revision.set(project["revision"].to_string());
                    let policy:Value=serde_json::from_str(&{text(&project,"creationPolicy")}).unwrap_or(Value::Null);
                    default_allow.set(policy["defaultAllow"]==true);
                    allow.set(rows(&policy,"allow").iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"));
                    deny.set(rows(&policy,"deny").iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"));
                },"Select project"}
            }
        }
        form{class:"hg-card hg-stack",onsubmit:move|e|{
            e.prevent_default();if busy(){return;}let id=space();busy.set(true);
            spawn(async move{match mutate("/projects/adopt","POST",json!({"spaceId":id.trim()})).await{
                Ok(_)=>{message.set("Project registered. Room access is allowed by default; deny entries take precedence.".into());projects.restart();},Err(e)=>message.set(e),}busy.set(false);});
        },h2{"Register an existing Space"}
            label{"Space ID" input{class:"hg-input",required:true,value:space(),placeholder:"!space:server",oninput:move|e|space.set(e.value())}}
            button{class:"hg-button",disabled:busy(),r#type:"submit","Register project"}
        }
        form{class:"hg-card hg-stack",onsubmit:move|e|{
            e.prevent_default();if busy(){return;}let id=project_id();let room_id=room();busy.set(true);
            spawn(async move{match mutate(&format!("/projects/{id}/rooms/adopt"),"POST",json!({"roomId":room_id.trim()})).await{Ok(_)=>message.set("Room registered. Its Matrix membership remains independent.".into()),Err(e)=>message.set(e)}busy.set(false);});
        },h2{"Register a Room in this Space"}
            label{"Project ID" input{class:"hg-input",required:true,value:project_id(),oninput:move|e|project_id.set(e.value())}}
            label{"Room ID" input{class:"hg-input",required:true,value:room(),placeholder:"!room:server",oninput:move|e|room.set(e.value())}}
            button{class:"hg-button",disabled:busy(),r#type:"submit","Register Room"}
        }
        super::room_discovery::RoomDiscovery { project_id, room }
        super::project_controls::ProjectControls { project_id, room }
        form{class:"hg-card hg-stack",onsubmit:move|e|{
            e.prevent_default();if busy(){return;}let Ok(expected_revision)=revision().parse::<i64>()else{message.set("Select a project with a valid revision.".into());return;};
            let id=project_id();let policy=json!({"defaultAllow":default_allow(),"allow":members(&allow()),"deny":members(&deny())});busy.set(true);
            spawn(async move{match mutate(&format!("/projects/{id}/creation-policy"),"PUT",json!({"expectedRevision":expected_revision,"policy":policy})).await{
                Ok(_)=>{message.set("Room access policy saved. This policy does not stop existing bindings.".into());projects.restart();},Err(e)=>message.set(e)}busy.set(false);});
        },h2{"Project Room access rights"}
            p{"The server checks your current Space administration rights. Deny entries override default permission and explicit allow entries."}
            label{"Current revision" input{class:"hg-input",required:true,value:revision(),oninput:move|e|revision.set(e.value())}}
            label{input{r#type:"checkbox",checked:default_allow(),onchange:move|e|default_allow.set(e.checked())}" Members may connect agents to Rooms by default"}
            label{"Allowed Matrix users (when default permission is off)" textarea{class:"hg-input",value:allow(),oninput:move|e|allow.set(e.value())}}
            label{"Denied Matrix users" textarea{class:"hg-input",value:deny(),oninput:move|e|deny.set(e.value())}}
            button{class:"hg-button",disabled:busy()||project_id().is_empty(),r#type:"submit","Save Room access policy"}
        }
    }}
}
