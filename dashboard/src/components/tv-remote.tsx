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

const small = "max-phone:**:data-[icon-only=true]:size-11"

// Inputs and menus: icon keys in a row, narrower on a phone.
const row = `flex flex-wrap justify-center gap-2 max-phone:gap-1.5 **:data-[icon-only=true]:size-13 ${small}`

const menus = `flex w-88 flex-wrap justify-between gap-2 max-phone:w-73.5 max-phone:gap-1.5 **:data-[icon-only=true]:size-13 ${small}`

// A pill-shaped track with the keys spread from end to end, like a remote's rocker.
const rocker =
  "flex flex-col flex-nowrap justify-between gap-1 rounded-full bg-ground p-1 **:data-[slot=button]:size-14 **:data-[slot=button]:hover:bg-clip-border max-phone:**:data-[slot=button]:size-12"

// Four arrows on a disc of tone around a solid OK.
const dpad =
  "grid w-max grid-cols-remote-dpad gap-2 grid-rows-remote-dpad place-items-center rounded-full bg-ground p-2 **:data-[icon-only=true]:hover:bg-clip-border max-phone:grid-cols-remote-dpad-snug max-phone:grid-rows-remote-dpad-snug **:data-[icon-only=true]:size-13 max-phone:**:data-[icon-only=true]:size-12 **:data-[command=ok]:size-17 max-phone:**:data-[command=ok]:size-15 **:data-[command=ok]:bg-clip-border **:data-[command=ok]:bg-ink **:data-[command=ok]:text-body-lg **:data-[command=ok]:font-semibold **:data-[command=ok]:text-ground **:data-[command=up]:col-start-2 **:data-[command=up]:row-start-1 **:data-[command=left]:col-start-1 **:data-[command=left]:row-start-2 **:data-[command=ok]:col-start-2 **:data-[command=ok]:row-start-2 **:data-[command=right]:col-start-3 **:data-[command=right]:row-start-2 **:data-[command=down]:col-start-2 **:data-[command=down]:row-start-3"

const keypad =
  "grid w-88 grid-cols-remote-keypad justify-between gap-2 max-phone:w-73.5 max-phone:grid-cols-remote-keypad-snug max-phone:gap-1.5 **:data-[slot=button]:size-13 max-phone:**:data-[slot=button]:size-11 **:data-[slot=button]:text-large **:data-[slot=button]:tabular-nums"

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
  quickGroups: [{ label: "TV power", commands: ["power"] }],
  // The TV plate is a little taller than its neighbours on both pages.
  plate: "py-4",
  panel: "rounded-4xl p-flow py-4",
  // Inputs across the top, the d-pad between volume (left) and channels (right), then the
  // menus over the numbers; the lower rows share one width and key size, so their edges line up.
  groups:
    "grid w-full flex-1 grid-cols-remote-triple content-evenly items-center justify-center gap-x-remote-gap gap-y-3 max-phone:gap-x-2.5",
  // The glyph stays level with the speaker's despite the tighter panel.
  identity: "mt-2",
  fullGroups: [
    {
      label: "Power and input",
      place: "col-span-3 col-start-1 row-start-1 justify-self-center",
      keys: row,
      commands: ["power", "input", "hdmi-1", "hdmi-2", "hdmi-3"],
    },
    {
      label: "Volume",
      place: "col-start-1 row-start-2 flex self-stretch justify-self-center",
      keys: rocker,
      commands: ["volume-up", "mute", "volume-down"],
    },
    {
      label: "Navigation",
      place: "col-start-2 row-start-2 justify-self-center",
      keys: dpad,
      commands: ["up", "left", "ok", "right", "down"],
    },
    {
      label: "Menus",
      place: "col-span-3 col-start-1 row-start-3 justify-self-center",
      keys: menus,
      commands: ["back", "home", "exit", "settings", "info", "guide"],
    },
    {
      label: "Channels",
      place: "col-start-3 row-start-2 flex self-stretch justify-self-center",
      keys: rocker,
      commands: ["channel-up", "channel-down"],
    },
    {
      label: "Numbers",
      place: "col-span-3 col-start-1 row-start-4 justify-self-center",
      keys: keypad,
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
