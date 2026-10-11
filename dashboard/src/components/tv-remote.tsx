import { CaretDownIcon } from "@phosphor-icons/react/dist/csr/CaretDown"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { CaretUpIcon } from "@phosphor-icons/react/dist/csr/CaretUp"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { PowerIcon } from "@phosphor-icons/react/dist/csr/Power"
import { SpeakerSimpleSlashIcon } from "@phosphor-icons/react/dist/csr/SpeakerSimpleSlash"
import { TelevisionSimpleIcon } from "@phosphor-icons/react/dist/csr/TelevisionSimple"
import {
  IrRemote,
  type IrRemoteConfig,
  type IrRemoteProps,
} from "@/components/ir-remote"
import { tvCommandLabels, type TvCommand } from "@/lib/tv"

const tvRemote: IrRemoteConfig<TvCommand> = {
  id: "tv",
  device: "TV",
  title: "TV",
  remoteName: "LG TV remote",
  icon: TelevisionSimpleIcon,
  primary: "power",
  labels: tvCommandLabels,
  commandIcons: {
    power: PowerIcon,
    mute: SpeakerSimpleSlashIcon,
    "volume-down": MinusIcon,
    "volume-up": PlusIcon,
    up: CaretUpIcon,
    down: CaretDownIcon,
    left: CaretLeftIcon,
    right: CaretRightIcon,
  },
  quickGroups: [
    {
      label: "Common TV commands",
      kind: "quick",
      commands: ["power", "mute", "volume-down", "volume-up"],
    },
  ],
  fullGroups: [
    {
      label: "Power and input",
      kind: "row",
      commands: ["power", "input", "hdmi-1", "hdmi-2", "hdmi-3"],
    },
    {
      label: "Volume",
      kind: "row",
      commands: ["volume-down", "mute", "volume-up"],
    },
    {
      label: "Navigation",
      kind: "dpad",
      commands: ["up", "left", "ok", "right", "down"],
    },
    {
      label: "Menus",
      kind: "row",
      commands: ["back", "home", "exit", "settings", "info", "guide"],
    },
    {
      label: "Channels",
      kind: "row",
      commands: ["channel-down", "channel-up"],
    },
    {
      label: "Numbers",
      kind: "keypad",
      commands: ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
    },
  ],
  unobserved: "Power and volume",
  explanation:
    "Commands are sent to the TV by infrared. OpenHome does not report whether the TV is on, its input or its volume.",
}

/** An LG TV remote whose buttons come from the commands OpenHome reports. */
export function TvRemote(props: IrRemoteProps<TvCommand>): React.JSX.Element {
  return <IrRemote config={tvRemote} {...props} />
}
