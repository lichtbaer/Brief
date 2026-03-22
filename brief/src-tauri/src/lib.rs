mod audio;
mod storage;
mod transcribe;

#[tauri::command]
async fn start_recording(meeting_type: String) -> Result<String, String> {
    let _ = meeting_type;
    Ok("placeholder-session-id".to_string())
}

#[tauri::command]
async fn stop_recording(session_id: String) -> Result<String, String> {
    let _ = session_id;
    Ok("placeholder-audio-path".to_string())
}

#[tauri::command]
async fn process_meeting(session_id: String, audio_path: String) -> Result<String, String> {
    let _ = (session_id, audio_path);
    Ok("{}".to_string())
}

#[tauri::command]
async fn get_meeting(id: String) -> Result<String, String> {
    let _ = id;
    Ok("{}".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            process_meeting,
            get_meeting
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
