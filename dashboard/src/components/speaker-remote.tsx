import { BluetoothIcon } from "@phosphor-icons/react/dist/csr/Bluetooth"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { PowerIcon } from "@phosphor-icons/react/dist/csr/Power"
import { SpeakerHifiIcon } from "@phosphor-icons/react/dist/csr/SpeakerHifi"
import { SpeakerSimpleSlashIcon } from "@phosphor-icons/react/dist/csr/SpeakerSimpleSlash"
import { WaveformIcon } from "@phosphor-icons/react/dist/csr/Waveform"
import {
  IrRemote,
  type IrRemoteConfig,
  type IrRemoteProps,
} from "@/components/ir-remote"
import { speakerCommandLabels, type SpeakerCommand } from "@/lib/speaker"

const speakerRemote: IrRemoteConfig<SpeakerCommand> = {
  id: "speaker",
  device: "speaker",
  title: "Speaker",
  remoteName: "Edifier speaker remote",
  icon: SpeakerHifiIcon,
  labels: speakerCommandLabels,
  commandIcons: {
    power: PowerIcon,
    bluetooth: BluetoothIcon,
    optical: WaveformIcon,
    mute: SpeakerSimpleSlashIcon,
    "volume-down": MinusIcon,
    "volume-up": PlusIcon,
  },
  // Choosing the input and adjusting volume are the everyday speaker actions.
  quickGroups: [
    {
      label: "Common speaker commands",
      kind: "quick",
      commands: ["bluetooth", "optical", "volume-down", "volume-up"],
    },
  ],
  // Three stacked buttons per row fit the speaker's narrow column on Controls.
  fullGroups: [
    {
      label: "Power and input",
      kind: "trio",
      commands: ["power", "bluetooth", "optical"],
    },
    {
      label: "Volume",
      kind: "trio",
      commands: ["volume-down", "mute", "volume-up"],
    },
  ],
  unobserved: "Power and volume",
  explanation:
    "Commands are sent to the speaker by infrared. OpenHome does not report whether the speaker is on, its input, its volume or what is playing.",
}

/** An Edifier speaker remote whose buttons come from the commands OpenHome reports. */
export function SpeakerRemote(
  props: IrRemoteProps<SpeakerCommand>
): React.JSX.Element {
  return <IrRemote config={speakerRemote} {...props} />
}
