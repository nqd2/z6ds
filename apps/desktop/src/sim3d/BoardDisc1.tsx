import { useLayoutEffect, useMemo, useRef } from "react";
import { useGLTF } from "@react-three/drei";
import type { ThreeEvent } from "@react-three/fiber";
import { Box3, Group, Vector3 } from "three";
import { BOARD_MODEL_URL, BOARD_TARGET_WIDTH } from "./boardModel";

type LedState = Record<string, boolean>;

interface BoardDisc1Props {
  gpio: Record<string, number>;
  ledVisual: LedState;
  onUserPress: () => void;
  onUserRelease: () => void;
  onReset: () => void;
}

function Led({
  position,
  on,
  color = "#00e676",
}: {
  position: [number, number, number];
  on: boolean;
  color?: string;
}) {
  return (
    <mesh position={position}>
      <sphereGeometry args={[0.045, 16, 16]} />
      <meshStandardMaterial
        color={on ? color : "#1a2332"}
        emissive={on ? color : "#000000"}
        emissiveIntensity={on ? 1.4 : 0}
      />
    </mesh>
  );
}

function BoardMesh() {
  const { scene } = useGLTF(BOARD_MODEL_URL);
  const model = useMemo(() => scene.clone(true), [scene]);
  return <primitive object={model} />;
}

export function BoardDisc1({
  gpio,
  ledVisual,
  onUserPress,
  onUserRelease,
  onReset,
}: BoardDisc1Props) {
  const fitRef = useRef<Group>(null);
  const modelRef = useRef<Group>(null);
  const pg13 = ledVisual.PG13 ?? (gpio.PG13 ?? 0) !== 0;
  const pg14 = ledVisual.PG14 ?? (gpio.PG14 ?? 0) !== 0;

  const stop = (e: ThreeEvent<PointerEvent | MouseEvent>) => {
    e.stopPropagation();
  };

  useLayoutEffect(() => {
    const modelGroup = modelRef.current;
    const fitGroup = fitRef.current;
    if (!modelGroup || !fitGroup) return;

    const box = new Box3().setFromObject(modelGroup);
    if (box.isEmpty()) return;

    const size = box.getSize(new Vector3());
    const footprint = Math.max(size.x, size.z);
    const scale = footprint > 0 ? BOARD_TARGET_WIDTH / footprint : 1;
    fitGroup.scale.setScalar(scale);

    const scaled = new Box3().setFromObject(fitGroup);
    const center = scaled.getCenter(new Vector3());
    fitGroup.position.set(-center.x, -scaled.min.y, -center.z);
  }, []);

  return (
    <group>
      <group ref={fitRef}>
        <group ref={modelRef}>
          <BoardMesh />
        </group>

        {/* Overlays tuned for DISC1 layout on the GLB footprint (2.4u wide). */}
        <Led position={[-0.82, 0.12, 0.52]} on={pg13} color="#4ade80" />
        <Led position={[-0.62, 0.12, 0.52]} on={pg14} color="#f87171" />

        <mesh
          position={[0.72, 0.1, 0.42]}
          onPointerDown={(e) => {
            stop(e);
            onUserPress();
          }}
          onPointerUp={(e) => {
            stop(e);
            onUserRelease();
          }}
          onPointerLeave={(e) => {
            stop(e);
            onUserRelease();
          }}
        >
          <cylinderGeometry args={[0.09, 0.09, 0.05, 24]} />
          <meshStandardMaterial color="#334155" transparent opacity={0.35} />
        </mesh>

        <mesh
          position={[0.92, 0.1, 0.42]}
          onClick={(e) => {
            stop(e);
            onReset();
          }}
        >
          <boxGeometry args={[0.12, 0.05, 0.07]} />
          <meshStandardMaterial color="#64748b" transparent opacity={0.35} />
        </mesh>
      </group>
    </group>
  );
}

useGLTF.preload(BOARD_MODEL_URL);
