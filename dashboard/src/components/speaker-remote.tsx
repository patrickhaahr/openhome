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
  primary: "power",
  labels: speakerCommandLabels,
  commandIcons: {
    power: PowerIcon,
    bluetooth: BluetoothIcon,
    optical: WaveformIcon,
    mute: SpeakerSimpleSlashIcon,
    "volume-down": MinusIcon,
    "volume-up": PlusIcon,
  },
  plateIcons: {},
  plateIdentity: true,
  // Glyphs only, as on Home; each key keeps its name for assistive technology.
  worded: [],
  // Choosing the input and adjusting volume are the everyday speaker actions.
  quickGroups: [
    {
      label: "Common speaker commands",
      // Volume up sits above volume down in Home's vertical plate.
      commands: ["bluetooth", "optical", "volume-up", "volume-down"],
    },
  ],
  plate: "",
  panel: "rounded-4xl p-flow",
  groups: "flex w-full flex-1 flex-col items-center justify-evenly gap-5",
  identity: "",
  // The speaker's narrow plate on Controls: power over its inputs, then a rocker like the TV's.
  fullGroups: [
    {
      label: "Power and input",
      place: "flex w-full justify-center",
      keys: "grid grid-cols-pair gap-3 **:data-[command=power]:col-span-full **:data-[command=power]:mb-2 **:data-[command=power]:justify-self-center",
      commands: ["power", "bluetooth", "optical"],
    },
    {
      label: "Volume",
      place: "flex w-full justify-center",
      keys: "flex flex-col flex-nowrap gap-1 rounded-full bg-ground p-1 **:data-[slot=button]:size-14 **:data-[slot=button]:hover:bg-clip-border",
      commands: ["volume-up", "mute", "volume-down"],
    },
  ],
  // Input matters most for the speaker, so the label covers every unreported state.
  unobserved: "Observed state",
  explanation:
    "Commands are sent to the speaker by infrared. OpenHome does not report whether the speaker is on, its input, its volume or what is playing.",
}

/** An Edifier speaker remote whose buttons come from the commands OpenHome reports. */
export function SpeakerRemote(
  props: IrRemoteProps<SpeakerCommand>
): React.JSX.Element {
  return <IrRemote config={speakerRemote} {...props} />
}
