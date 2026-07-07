use leptos::*;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;

// 1. THE BRIDGE: This lets our frontend call the backend Tauri commands
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

// 2. THE DATA: Matching the structs from our backend
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CloudBuild {
    pub id: String,
    pub name: String,
    pub date: String,
    pub is_compiled: bool,
}

#[derive(Serialize, Deserialize)]
struct GetBuildsArgs {
    channel_type: String,
}

// 3. THE UI COMPONENT
#[component]
pub fn app() -> impl IntoView {
    // Reactive variables to hold our data and loading state
    let (builds, set_builds) = create_signal(Vec::<CloudBuild>::new());
    let (is_loading, set_loading) = create_signal(false);

    // Function triggered when the user clicks "Fetch Dev Builds"
    let fetch_builds = move |_| {
        set_loading.set(true);
        
        spawn_local(async move {
            let args = to_value(&GetBuildsArgs {
                channel_type: "dev".to_string(),
            }).unwrap();

            // Call the backend command we wrote in src-tauri/src/lib.rs
            let res = unsafe { invoke("get_cloud_builds", args) }.await;
            
            if let Ok(parsed_builds) = serde_wasm_bindgen::from_value::<Vec<CloudBuild>>(res) {
                set_builds.set(parsed_builds);
            }
            
            set_loading.set(false);
        });
    };

    view! {
        <main style="display: flex; height: 100vh; background-color: #121214; color: #ffffff; font-family: sans-serif;">
            
            // SIDEBAR
            <aside style="width: 250px; background-color: #1c1c1f; padding: 20px; border-right: 1px solid #2d2d30;">
                <h2 style="font-size: 18px; margin-bottom: 20px;">"ChroMapper Manager"</h2>
                
                <button 
                    on:click=fetch_builds
                    style="width: 100%; padding: 10px; background-color: #3b82f6; color: white; border: none; border-radius: 6px; cursor: pointer;"
                >
                    {move || if is_loading.get() { "Scraping Jenkins..." } else { "Fetch Dev Builds" }}
                </button>
            </aside>

            // MAIN CONTENT AREA
            <section style="flex-grow: 1; padding: 30px; overflow-y: auto;">
                <h1>"Available Versions"</h1>
                
                <div style="display: flex; flex-direction: column; gap: 15px; margin-top: 20px;">
                    // Loop through the builds and create a card for each one
                    {move || builds.get().into_iter().map(|build| {
                        
                        // Change UI slightly if the build failed to compile
                        let border_color = if build.is_compiled { "#2d2d30" } else { "#ef4444" };
                        let status_text = if build.is_compiled { "Ready" } else { "⚠️ Compilation Failed" };

                        view! {
                            <div style=format!("background-color: #1c1c1f; padding: 15px; border-radius: 8px; border: 1px solid {}; display: flex; justify-content: space-between; align-items: center;", border_color)>
                                <div>
                                    <h3 style="margin: 0 0 5px 0;">{build.name}</h3>
                                    <span style="font-size: 12px; color: #9ca3af;">"Released: " {build.date}</span>
                                </div>
                                <div>
                                    <span style="font-size: 14px; margin-right: 15px;">{status_text}</span>
                                    <button 
                                        disabled=!build.is_compiled
                                        style="padding: 8px 15px; background-color: #22c55e; color: white; border: none; border-radius: 4px; cursor: pointer;"
                                    >
                                        "Install"
                                    </button>
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>
            
        </main>
    }
}