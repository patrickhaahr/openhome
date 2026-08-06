/** A named command exposed by an IR remote. */
export type RemoteCommand = {
  readonly command: string;
  readonly label: string;
};

/** Edifier commands shown on the Home tab. */
export const homeRemoteRows: ReadonlyArray<ReadonlyArray<RemoteCommand>> = [
  [
    { command: 'power', label: 'Power' },
    { command: 'bluetooth', label: 'Bluetooth' },
    { command: 'optical', label: 'Optical' },
  ],
  [
    { command: 'mute', label: 'Mute' },
    { command: 'volume-down', label: 'Volume -' },
    { command: 'volume-up', label: 'Volume +' },
  ],
];

/** LG TV commands shown on the Remote tab. Null entries preserve directional layout. */
export const tvRemoteRows: ReadonlyArray<ReadonlyArray<RemoteCommand | null>> = [
  [{ command: 'power', label: 'Power' }, { command: 'input', label: 'Input' }],
  [{ command: 'volume-up', label: 'Volume +' }, { command: 'mute', label: 'Mute' }, { command: 'volume-down', label: 'Volume -' }],
  [{ command: 'channel-up', label: 'Channel +' }, { command: 'guide', label: 'Guide' }, { command: 'channel-down', label: 'Channel -' }],
  [{ command: 'home', label: 'Home' }, { command: 'settings', label: 'Settings' }, { command: 'info', label: 'Info' }],
  [null, { command: 'up', label: 'Up' }, null],
  [{ command: 'left', label: 'Left' }, { command: 'ok', label: 'OK' }, { command: 'right', label: 'Right' }],
  [{ command: 'back', label: 'Back' }, { command: 'down', label: 'Down' }, { command: 'exit', label: 'Exit' }],
  [{ command: '1', label: '1' }, { command: '2', label: '2' }, { command: '3', label: '3' }],
  [{ command: '4', label: '4' }, { command: '5', label: '5' }, { command: '6', label: '6' }],
  [{ command: '7', label: '7' }, { command: '8', label: '8' }, { command: '9', label: '9' }],
  [null, { command: '0', label: '0' }, null],
  [{ command: 'hdmi-1', label: 'HDMI 1' }, { command: 'hdmi-2', label: 'HDMI 2' }, { command: 'hdmi-3', label: 'HDMI 3' }],
];
