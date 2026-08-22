import sys
import json
import warnings

warnings.filterwarnings("ignore")

import scenedetect
from scenedetect import open_video, SceneManager, AdaptiveDetector

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "Missing video path argument"}))
        sys.exit(1)
    
    video_path = sys.argv[1]
    
    try:
        video = open_video(video_path)
        scene_manager = SceneManager()
        scene_manager.add_detector(AdaptiveDetector())
        scene_manager.detect_scenes(video)
        scene_list = scene_manager.get_scene_list()
        
        cuts = []
        scenes = []
        for i, scene in enumerate(scene_list):
            start_sec = float(scene[0].get_seconds())
            end_sec = float(scene[1].get_seconds())
            scenes.append({"start": round(start_sec, 6), "end": round(end_sec, 6)})
            if i > 0:
                cuts.append(round(start_sec, 6))
        
        output = {
            "cuts": cuts,
            "scenes": scenes,
            "count": len(cuts)
        }
        print(json.dumps(output))
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
