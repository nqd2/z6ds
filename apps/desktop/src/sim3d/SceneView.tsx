import { Suspense } from "react";
import { Canvas } from "@react-three/fiber";
import { ContactShadows, Environment, OrbitControls } from "@react-three/drei";
import { BoardDisc1 } from "./BoardDisc1";
import "./SceneView.css";

interface SceneViewProps {
  gpio: Record<string, number>;
  ledVisual: Record<string, boolean>;
  sessionId: string | null;
  onUserPress: () => void;
  onUserRelease: () => void;
  onReset: () => void;
}

function SceneLoading() {
  return (
    <mesh>
      <boxGeometry args={[0.5, 0.05, 0.5]} />
      <meshStandardMaterial color="#334155" />
    </mesh>
  );
}

export function SceneView({
  gpio,
  ledVisual,
  sessionId,
  onUserPress,
  onUserRelease,
  onReset,
}: SceneViewProps) {
  return (
    <div className="scene-view" data-session={sessionId ?? "none"}>
      <Canvas camera={{ position: [3.2, 2.4, 3.2], fov: 42 }} shadows>
        <color attach="background" args={["#0b1220"]} />
        <ambientLight intensity={0.35} />
        <directionalLight
          position={[5, 8, 4]}
          intensity={1.15}
          castShadow
          shadow-mapSize={[1024, 1024]}
        />
        <Environment preset="city" />
        <Suspense fallback={<SceneLoading />}>
          <BoardDisc1
            gpio={gpio}
            ledVisual={ledVisual}
            onUserPress={onUserPress}
            onUserRelease={onUserRelease}
            onReset={onReset}
          />
        </Suspense>
        <ContactShadows
          position={[0, 0, 0]}
          opacity={0.45}
          scale={12}
          blur={2.5}
          far={6}
        />
        <OrbitControls
          makeDefault
          enablePan
          enableZoom
          minPolarAngle={0.25}
          maxPolarAngle={Math.PI / 2 - 0.05}
          target={[0, 0.35, 0]}
        />
      </Canvas>
      <div className="scene-hint">Orbit · zoom · USER / RESET</div>
    </div>
  );
}
