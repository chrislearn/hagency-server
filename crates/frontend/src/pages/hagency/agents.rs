use super::common::*;
use dioxus::prelude::*;
use serde_json::{Value, json};
#[component]
pub fn Agents() -> Element {
    let mut agents = use_resource(|| async {
        browser_auth::call("/agents", "GET", None)
            .await
            .map_err(|e| e.message)
    });
    let mut selected = use_signal(String::new);
    let mut bindings = use_resource(move || async move {
        let id = selected();
        if id.is_empty() {
            return Ok(json!({"bindings":[]}));
        }
        browser_auth::call(&format!("/agents/{id}/bindings"), "GET", None)
            .await
            .map_err(|e| e.message)
    });
    let mut message = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut name = use_signal(String::new);
    let mut project = use_signal(String::new);
    let mut room = use_signal(String::new);
    let mut command = use_signal(browser_auth::operation_id);
    let data = agents().and_then(Result::ok).unwrap_or(Value::Null);
    rsx! {div{class:"hg-page hg-stack",
        h1{class:"text-2xl font-bold","My agents"}
        p{"You permanently own every agent you create. Your local hagency-client runs Codex and controls resource quotas and tool permissions."}
        Notice{message}
        button{class:"hg-button",onclick:move|_|{agents.restart();bindings.restart();},"Refresh"}
        if let Some(Err(error))=agents(){p{role:"alert","{error}"}}
        for agent in rows(&data,"agents") {
            div{class:"hg-card hg-stack",key:"{agent}",
                strong{{text(&agent,"displayName")}} code{{text(&agent,"puppetMxid")}}
                p{{format!("State: {}",agent["state"])}}
                button{class:"hg-button",onclick:move|_|{selected.set(text(&agent,"id"));command.set(browser_auth::operation_id());},"Select agent"}
            }
        }
        form{class:"hg-card hg-stack",onsubmit:move|e|{
            e.prevent_default();if busy(){return;}let id=selected();let body=if id.is_empty(){json!({"projectId":project().trim(),"roomId":room().trim(),"displayName":name().trim(),"idempotencyKey":command()})}else{json!({"projectId":project().trim(),"roomId":room().trim(),"idempotencyKey":command()})};
            let path=if id.is_empty(){"/agents".into()}else{format!("/agents/{id}/bindings")};busy.set(true);
            spawn(async move{match mutate(&path,"POST",body).await{
                Ok(result)=>{message.set(result);command.set(browser_auth::operation_id());agents.restart();bindings.restart();},Err(e)=>message.set(e)}busy.set(false);});
        },h2{if selected().is_empty(){"Create an agent"}else{"Add a Room binding"}}
            p{"Join the Space and Room first. Register the Room in the Project and invite the Hagency service account so it can invite the puppet. Encrypted Rooms wait for client crypto support."}
            label{"Project ID" input{class:"hg-input",required:true,value:project(),oninput:move|e|{project.set(e.value());command.set(browser_auth::operation_id());}}}
            label{"Room ID" input{class:"hg-input",required:true,value:room(),oninput:move|e|{room.set(e.value());command.set(browser_auth::operation_id());}}}
            if selected().is_empty(){label{"Agent name" input{class:"hg-input",required:true,value:name(),oninput:move|e|{name.set(e.value());command.set(browser_auth::operation_id());}}}}
            button{class:"hg-button",disabled:busy(),r#type:"submit",if selected().is_empty(){"Create agent"}else{"Bind Room"}}
            if !selected().is_empty(){button{class:"hg-link",r#type:"button",onclick:move|_|{selected.set(String::new());command.set(browser_auth::operation_id());},"Create a different agent"}}
        }
        if !selected().is_empty(){div{class:"hg-card hg-stack",h2{"Selected agent"}code{"{selected}"}
            for action in ["pause","resume"]{button{class:"hg-button",disabled:busy(),onclick:move|_|{let id=selected();busy.set(true);spawn(async move{match mutate(&format!("/agents/{id}/{action}"),"POST",json!({})).await{Ok(_)=>{message.set(format!("Agent {action} accepted."));agents.restart();},Err(e)=>message.set(e)}busy.set(false);});},"{action}"}}
            if let Some(Err(error))=bindings(){p{role:"alert","{error}"}}
            for binding in rows(&bindings().and_then(Result::ok).unwrap_or(Value::Null),"bindings"){
                div{class:"hg-card",code{{text(&binding,"roomId")}}p{{format!("Project {}; state {}",binding["projectId"],binding["state"])}}
                    for action in ["pause","resume","leave"]{
                        {let binding_id={text(&binding,"id")};rsx!{button{class:"hg-button",disabled:busy(),onclick:move|_|{
                            let id=binding_id.clone();busy.set(true);spawn(async move{let (path,method)=if action=="leave"{(format!("/bindings/{id}"),"DELETE")}else{(format!("/bindings/{id}/{action}"),"POST")};
                            match mutate(&path,method,json!({})).await{Ok(_)=>{message.set(format!("Binding {action} accepted."));bindings.restart();},Err(e)=>message.set(e)}busy.set(false);});
                        },"{action}"}}}
                    }
                }
            }
        }}
    }}
}
