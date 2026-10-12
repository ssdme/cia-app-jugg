import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { CustomParams, EffectOverrides, EffectPreview, MediaInfo, PreparedScene, RenderStats, SceneClip, Transcript } from './contracts';
interface Responses {
  get_app_version: string;
  cmd_get_style_preset: { effects: EffectOverrides; params: CustomParams };
  cmd_get_style_defaults: CustomParams;
  get_effect_previews: EffectPreview[];
  media_fingerprints: string[];
  probe_media: MediaInfo;
  get_multi_scene_clips: SceneClip[];
  get_scene_clips: SceneClip[];
  detect_scenes: number[];
  prepare_scene_selection: PreparedScene[];
  cancel_scene_analysis: void;
  detect_beats: { beats: number[]; downbeats: number[]; bpm: number };
  generate_plan: string; save_plan: string;
  run_render_pipeline: RenderStats; cancel_render: void;
  open_target_folder: void; open_output_file: void; open_about_link: void;
  pick_file: string | null; pick_files: string[] | null;
  read_media_file_bytes: number[];
  transcribe_audio: Transcript; render_text_video: string;
}
/** All native response shapes used by components live in one checked contract. */
export function invoke<K extends keyof Responses>(command: K, args?: Record<string, unknown>): Promise<Responses[K]> {
  return tauriInvoke<Responses[K]>(command, args);
}
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return typeof error === 'string' ? error : String(error);
}
