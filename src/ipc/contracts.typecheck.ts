import type { WannierInfo } from '../types/crystal';
import type { IpcArgs } from './contracts';

declare const info: WannierInfo;

const translation = info.r_shells[0];
const [rx, ry, rz] = translation;
void rx;
void ry;
void rz;

// @ts-expect-error Wannier translations are serialized as integer triplets, not objects.
void translation.rx;

const pick_args: IpcArgs<'pick_atom'> = { x: 0, y: 0, screenW: 800, screenH: 600 };
void pick_args;

// @ts-expect-error Rust `screen_h` maps to frontend `screenH`, never `screenHeight`.
const invalid_pick_args: IpcArgs<'pick_atom'> = { x: 0, y: 0, screenW: 800, screenHeight: 600 };
void invalid_pick_args;
