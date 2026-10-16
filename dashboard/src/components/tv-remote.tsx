import { ArrowUUpLeftIcon } from "@phosphor-icons/react/dist/csr/ArrowUUpLeft"
import { CaretDoubleDownIcon } from "@phosphor-icons/react/dist/csr/CaretDoubleDown"
import { CaretDoubleUpIcon } from "@phosphor-icons/react/dist/csr/CaretDoubleUp"
import { CaretDownIcon } from "@phosphor-icons/react/dist/csr/CaretDown"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { CaretUpIcon } from "@phosphor-icons/react/dist/csr/CaretUp"
import { GearSixIcon } from "@phosphor-icons/react/dist/csr/GearSix"
import { HouseIcon } from "@phosphor-icons/react/dist/csr/House"
import { InfoIcon } from "@phosphor-icons/react/dist/csr/Info"
import { ListBulletsIcon } from "@phosphor-icons/react/dist/csr/ListBullets"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { PowerIcon } from "@phosphor-icons/react/dist/csr/Power"
import { SpeakerSimpleSlashIcon } from "@phosphor-icons/react/dist/csr/SpeakerSimpleSlash"
import { SignOutIcon } from "@phosphor-icons/react/dist/csr/SignOut"
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
    back: ArrowUUpLeftIcon,
    home: HouseIcon,
    exit: SignOutIcon,
    settings: GearSixIcon,
    info: InfoIcon,
    guide: ListBulletsIcon,
    "channel-up": CaretDoubleUpIcon,
    "channel-down": CaretDoubleDownIcon,
  },
  // Home's lone power key wears the TV, so the plate needs no other name.
  plateIcons: { power: TelevisionSimpleIcon },
  plateIdentity: false,
  worded: [],
  quickGroups: [
    {
      label: "TV power",
      kind: "quick",
      commands: ["power"],
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
      kind: "rocker",
      commands: ["volume-up", "mute", "volume-down"],
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
      kind: "rocker",
      commands: ["channel-up", "channel-down"],
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
