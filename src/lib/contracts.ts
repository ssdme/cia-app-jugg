export interface EffectOverrides {
  shakes: boolean; zoom: boolean; flicker: boolean; oneFramers: boolean;
  transitions: boolean; tint: boolean; vignette: boolean; scanlines: boolean;
  echoTrail: boolean; exposureFlash: boolean; bouncyShake: boolean;
  dissolveShake: boolean; skewShake: boolean; squishPop: boolean;
  opticsBounce: boolean; buildupChain: boolean; warpStretch: boolean;
  zoomBeatOffset: boolean; ccDeepDark: boolean; antiFlash: boolean;
}
export interface CreativeOptions {
  timeSlice: boolean; inkPulse: boolean; prismDrift: boolean;
  focusPull: boolean; reliefLight: boolean; paperGate: boolean;
}
export interface MediaInfo {
  duration: number; width: number; height: number; fps: number;
  audioChannels: number; audioSampleRate: number;
}
export interface SceneClip {
  index: number; startTime: number; endTime: number; duration: number; thumbnail: string;
  sourceId: number; sourceDuration: number; usableRanges: SourceRange[];
}
export interface SourceRange { sourceId: number; start: number; duration: number }
export interface SceneSelection { sceneId: number; sourceId: number; start: number; end: number }
export interface MotionSample { time: number; activity: number; confidence: number }
export interface PreparedScene {
  sceneId: number; sourceId: number; sourceStart: number; sourceEnd: number;
  fingerprint: string; fps: number; fragments: SourceRange[]; motion: MotionSample[];
  actions: { start: number; end: number; peak: number; activity: number }[];
}
export interface AnalysisProgress { jobId: string; percent: number; message: string }
export interface RenderProgress {
  jobId: string; phase: string; percent: number;
  currentFrame: number; totalFrames: number; message: string;
}
export interface CustomParams {
  shakeA0: number; shakeOmega: number; shakeK: number; bouncyAmplitude: number;
  dissolvePct: number; skewS0: number; squishScaleYMin: number; squishScaleXMax: number;
  opticsK0: number; stretchScale: number; zoomScaleStart: number; zoomScaleEnd: number;
  zoomBeatOffsetFrames: number; flickerAmplitude: number; flickerFrequencyHz: number;
  exposureFlashPeak: number; echoAlpha: number; echoKDepth: number;
  tintROffset: number; tintGOffset: number; tintBOffset: number; vignetteStrength: number;
  scanlinesOpacity: number; warpBubbleAmplitude: number; warpBubbleFrequency: number;
  waveWarpHeight: number; waveWarpSpeed: number; slideShakePixels: number;
}
export interface EffectPreview { id: string; name: string; category: string; previewDataUrl: string; description: string }
export interface RenderStats {
  outputPath: string; renderTimeSecs: number; fileSizeMb: number; targetFps: number;
  effectsCount: number; actualFps: number; decodedBytes: number; peakCacheBytes: number;
  decodeTimeSecs: number; samplingTimeSecs: number; finalizationTimeSecs: number;
  peakFrameRamBytes: number; frameCacheHits: number; frameCacheMisses: number;
}
export interface PlanSummary {
  segmentsCount: number; loops: number; targetDuration: number; savedPath: string;
  style: string; fps: number; aspect: string; motionBlur: boolean; fullFx: boolean;
  shakes: boolean; zoom: boolean; reverse: boolean; oneFramers: boolean;
  transitions: boolean; ambiance: boolean; echoTrail: boolean;
  export: { codec: string; bitrateMbps: number; format: string };
}
export interface TranscriptWord { word: string; start: number; end: number; probability?: number }
export interface Transcript { text: string; duration: number; segments: { start: number; end: number; text: string; words?: TranscriptWord[] }[] }
export interface TextElement { text: string; x: number; y: number; size: number; key: string; start?: number | null; end?: number | null; w: number; h: number }
export interface TextBlock { id: string; start: number; end: number; isValidated: boolean; layoutVariant: number; words: TranscriptWord[]; wordsRef: string[]; elements: TextElement[] }
