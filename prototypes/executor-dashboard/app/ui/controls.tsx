import { LightbulbIcon } from "@phosphor-icons/react/dist/csr/Lightbulb";
import { TelevisionSimpleIcon } from "@phosphor-icons/react/dist/csr/TelevisionSimple";
import { SpeakerHighIcon } from "@phosphor-icons/react/dist/csr/SpeakerHigh";
import { BluetoothIcon } from "@phosphor-icons/react/dist/csr/Bluetooth";
import { PowerIcon } from "@phosphor-icons/react/dist/csr/Power";
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus";
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus";

/** Local prototype state; no commands are sent to household devices. */
export type DemoControls = {
  readonly lights: boolean;
  readonly tv: boolean;
  readonly speakerInput: "optical" | "bluetooth";
  readonly speakerVolume: number;
};

export type ControlAction =
  | { readonly kind: "lights"; readonly enabled: boolean }
  | { readonly kind: "tv-power" }
  | { readonly kind: "speaker-input"; readonly input: DemoControls["speakerInput"] }
  | { readonly kind: "speaker-volume"; readonly change: -1 | 1 };

export type ControlsProps = {
  readonly controls: DemoControls;
  readonly control: (action: ControlAction) => void;
};

export function ControlsPanel({ controls, control }: ControlsProps): React.JSX.Element {
  return (
    <section className="card device-controls" aria-label="Home controls demo">
      <h2 className="widget-title">Home controls</h2>
      <div className="device-groups">
        <section className="device-group" aria-label="Lights">
          <h3>
            <LightbulbIcon size={20} /> Lights
          </h3>
          <div className="device-buttons">
            <button
              aria-label="Lights on"
              aria-pressed={controls.lights}
              onClick={() => control({ kind: "lights", enabled: true })}
            >
              ON
            </button>
            <button
              aria-label="Lights off"
              aria-pressed={!controls.lights}
              onClick={() => control({ kind: "lights", enabled: false })}
            >
              OFF
            </button>
          </div>
        </section>
        <section className="device-group" aria-label="TV">
          <h3>
            <TelevisionSimpleIcon size={20} /> TV
          </h3>
          <div className="device-buttons">
            <button
              aria-label="TV power"
              aria-pressed={controls.tv}
              onClick={() => control({ kind: "tv-power" })}
            >
              <PowerIcon size={18} /> POWER
            </button>
          </div>
        </section>
        <section className="device-group" aria-label="Speaker">
          <h3>
            <SpeakerHighIcon size={20} /> Speaker
          </h3>
          <div className="device-buttons speaker-buttons">
            <button
              aria-label="Speaker optical input"
              aria-pressed={controls.speakerInput === "optical"}
              onClick={() => control({ kind: "speaker-input", input: "optical" })}
            >
              <TelevisionSimpleIcon size={20} />
            </button>
            <button
              aria-label="Speaker Bluetooth input"
              aria-pressed={controls.speakerInput === "bluetooth"}
              onClick={() => control({ kind: "speaker-input", input: "bluetooth" })}
            >
              <BluetoothIcon size={20} />
            </button>
            <button
              aria-label="Speaker volume up"
              disabled={controls.speakerVolume === 100}
              onClick={() => control({ kind: "speaker-volume", change: 1 })}
            >
              <PlusIcon size={20} />
            </button>
            <button
              aria-label="Speaker volume down"
              disabled={controls.speakerVolume === 0}
              onClick={() => control({ kind: "speaker-volume", change: -1 })}
            >
              <MinusIcon size={20} />
            </button>
          </div>
          <meter
            className="speaker-level"
            aria-label="Speaker volume"
            min={0}
            max={100}
            value={controls.speakerVolume}
          />
        </section>
      </div>
    </section>
  );
}
