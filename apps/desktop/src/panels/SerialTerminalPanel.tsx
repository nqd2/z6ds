import { useCallback, useEffect, useRef, useState } from "react";
import type { SimulatorSessionState, UartStreamChunk } from "../lib/contracts";
import { EventTypes, SCHEMA_VERSION } from "../lib/contracts";
import { useAppEvent } from "../hooks/useAppEvent";
import { hostSendUart } from "../lib/tauriApi";
import "./SerialTerminalPanel.css";

function decodeBase64(b64: string): string {
  try {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
    return new TextDecoder("utf-8", { fatal: false }).decode(bytes);
  } catch {
    return "";
  }
}

function encodeBase64(text: string): string {
  return btoa(
    new TextEncoder()
      .encode(text)
      .reduce((s, b) => s + String.fromCharCode(b), ""),
  );
}

interface SerialTerminalPanelProps {
  session: SimulatorSessionState | null;
}

export function SerialTerminalPanel({ session }: SerialTerminalPanelProps) {
  const [lines, setLines] = useState<string[]>([]);
  const [input, setInput] = useState("");
  const [connected, setConnected] = useState(false);
  const [lineEnding, setLineEnding] = useState<"CRLF" | "LF" | "CR" | "none">("CRLF");
  const logRef = useRef<HTMLPreElement>(null);

  useAppEvent(EventTypes.simulatorStarted, () => {
    setConnected(true);
    setLines((prev) => [...prev, "[serial] connected USART1 115200 8N1"]);
  });

  useAppEvent(EventTypes.simulatorStopped, () => {
    setConnected(false);
    setLines((prev) => [...prev, "[serial] disconnected"]);
  });

  useAppEvent(EventTypes.uartBridgeConnected, () => setConnected(true));

  useAppEvent<UartStreamChunk>(EventTypes.uartRx, (ev) => {
    const text = decodeBase64(ev.payload.bytesBase64);
    if (text) {
      setLines((prev) => [...prev, text]);
    }
  });

  useEffect(() => {
    logRef.current?.scrollTo(0, logRef.current.scrollHeight);
  }, [lines]);

  const send = useCallback(async () => {
    if (!session?.sessionId || !input.trim()) return;
    let payload = input;
    if (lineEnding === "CRLF") payload += "\r\n";
    else if (lineEnding === "LF") payload += "\n";
    else if (lineEnding === "CR") payload += "\r";
    await hostSendUart({
      schemaVersion: SCHEMA_VERSION,
      sessionId: session.sessionId,
      portId: "USART1",
      bytesBase64: encodeBase64(payload),
    });
    setLines((prev) => [...prev, `> ${input}`]);
    setInput("");
  }, [session, input, lineEnding]);

  return (
    <div className="serial-panel">
      <div className="serial-toolbar">
        <span className={connected ? "serial-ok" : "serial-off"}>
          {connected ? "USART1 connected" : "USART1 idle"}
        </span>
        <label>
          Line ending
          <select
            value={lineEnding}
            onChange={(e) =>
              setLineEnding(e.target.value as "CRLF" | "LF" | "CR" | "none")
            }
          >
            <option value="CRLF">CRLF</option>
            <option value="LF">LF</option>
            <option value="CR">CR</option>
            <option value="none">none</option>
          </select>
        </label>
        <button type="button" onClick={() => setLines([])}>
          Clear
        </button>
      </div>
      <pre ref={logRef} className="serial-log">
        {lines.length === 0 ? (
          <span className="serial-placeholder">RX from firmware appears here…</span>
        ) : (
          lines.join("")
        )}
      </pre>
      <div className="serial-input-row">
        <input
          type="text"
          value={input}
          disabled={!connected}
          placeholder={connected ? "Type and Enter to send" : "Start simulator first"}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void send();
          }}
        />
        <button type="button" disabled={!connected} onClick={() => void send()}>
          Send
        </button>
      </div>
    </div>
  );
}
