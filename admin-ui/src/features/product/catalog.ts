/// Names, links and logos for the user-facing pages. Product, app and agent
/// names are proper nouns and stay untranslated.

import meshyLogo from '../../assets/product/brand/meshy-logo-green.svg';
import meshyIcon from '../../assets/product/brand/meshy-icon-render-64.svg';
import blender from '../../assets/product/dcc/Blender-color.svg';
import unity from '../../assets/product/dcc/Unity-icon.svg';
import godot from '../../assets/product/dcc/GODOT-color.svg';
import unreal from '../../assets/product/dcc/Unreal-color.svg';
import maya from '../../assets/product/dcc/Maya-color.svg';
import max from '../../assets/product/dcc/3DSMax-color.svg';
import omniverse from '../../assets/product/dcc/Omniverse-color.svg';
import roblox from '../../assets/product/dcc/roblox-white.svg';
import zbrush from '../../assets/product/dcc/ZBrush-color.svg';
import houdini from '../../assets/product/dcc/Houdini-color.png';
import painter from '../../assets/product/dcc/SubstancePainter-color.png';
import freecad from '../../assets/product/dcc/FreeCAD-color.png';
import openscad from '../../assets/product/dcc/OpenSCAD-color.png';
import materialMaker from '../../assets/product/dcc/MaterialMaker-color.png';
import claudeCode from '../../assets/product/agents/claudecode-color.svg';
import codex from '../../assets/product/agents/codex-color.svg';
import cursor from '../../assets/product/agents/cursor.svg';
import geminiCli from '../../assets/product/agents/geminicli-color.svg';
import copilot from '../../assets/product/agents/githubcopilot.svg';
import opencode from '../../assets/product/agents/opencode.svg';
import windsurf from '../../assets/product/agents/windsurf.svg';
import kiro from '../../assets/product/agents/kiro-color.svg';
import trae from '../../assets/product/agents/trae-color.svg';
import qwen from '../../assets/product/agents/qwen-color.svg';
import codebuddy from '../../assets/product/agents/codebuddy-color.svg';
import openclaw from '../../assets/product/agents/openclaw-color.svg';
import hermes from '../../assets/product/agents/hermesagent.svg';
import cline from '../../assets/product/agents/cline.svg';
import goose from '../../assets/product/agents/goose.svg';
import antigravity from '../../assets/product/agents/antigravity-color.svg';
import pi from '../../assets/product/agents/pi.svg';
import amp from '../../assets/product/agents/amp-color.svg';
import kimi from '../../assets/product/agents/kimi.svg';

export const PRODUCT_NAME = 'Meshy DCC Core';
export const PRODUCT_SHORT_NAME = 'DCC Core';
export const WEBSITE_URL = 'https://www.meshy.ai';
export const MESHY_LOGO = meshyLogo;
export const MESHY_ICON = meshyIcon;

/// `mono` logos are white; the light theme inverts them.
export type Logo = { src: string; mono?: boolean };

export const DCC_NAMES: Record<string, string> = {
  blender: 'Blender',
  unity: 'Unity',
  godot: 'Godot',
  unreal: 'Unreal Engine',
  maya: 'Maya',
  '3dsmax': '3ds Max',
  omniverse: 'Omniverse',
  roblox: 'Roblox Studio',
  houdini: 'Houdini',
  c4d: 'Cinema 4D',
  zbrush: 'ZBrush',
  substance3d_painter: 'Substance 3D Painter',
  substance3d_designer: 'Substance 3D Designer',
  marmoset: 'Marmoset Toolbag',
  mari: 'Mari',
  katana: 'Katana',
  mobu: 'MotionBuilder',
  openusd: 'OpenUSD',
  sketchup: 'SketchUp',
  freecad: 'FreeCAD',
  openscad: 'OpenSCAD',
  'material-maker': 'Material Maker',
  photoshop: 'Photoshop',
};

/// Official logos only; a host without one is shown by name.
export const DCC_LOGOS: Record<string, Logo> = {
  blender: { src: blender },
  unity: { src: unity, mono: true },
  godot: { src: godot },
  unreal: { src: unreal },
  maya: { src: maya },
  '3dsmax': { src: max },
  omniverse: { src: omniverse },
  roblox: { src: roblox, mono: true },
  zbrush: { src: zbrush },
  houdini: { src: houdini },
  substance3d_painter: { src: painter },
  freecad: { src: freecad },
  openscad: { src: openscad },
  'material-maker': { src: materialMaker },
};

/// Minimum host versions the shipped connectors support.
export const DCC_REQUIREMENTS: Record<string, string> = {
  blender: 'Blender 4.5+',
  unity: 'Unity 2018.4+',
  godot: 'Godot 4.4+',
  unreal: 'Unreal Engine 5.0+',
  maya: 'Maya 2023+',
  zbrush: 'ZBrush 2026.1+',
  freecad: 'FreeCAD 1.0–1.1',
  'material-maker': 'Material Maker 1.7+',
  houdini: 'Houdini 20.5+',
  substance3d_painter: 'Substance 3D Painter 10.0+',
  openscad: 'OpenSCAD 2021.01, 2026.09–10 snapshots',
  c4d: 'Cinema 4D 2023+',
  '3dsmax': '3ds Max 2023+',
};

export const AGENT_LOGOS: Record<string, Logo> = {
  'claude-code': { src: claudeCode },
  codex: { src: codex },
  cursor: { src: cursor, mono: true },
  'gemini-cli': { src: geminiCli },
  'github-copilot': { src: copilot, mono: true },
  opencode: { src: opencode, mono: true },
  windsurf: { src: windsurf, mono: true },
  'kiro-cli': { src: kiro },
  trae: { src: trae },
  'trae-cn': { src: trae },
  'qwen-code': { src: qwen },
  codebuddy: { src: codebuddy },
  openclaw: { src: openclaw },
  'hermes-agent': { src: hermes, mono: true },
  cline: { src: cline, mono: true },
  goose: { src: goose, mono: true },
  antigravity: { src: antigravity },
  'antigravity-cli': { src: antigravity },
  pi: { src: pi, mono: true },
  amp: { src: amp },
  'kimi-code-cli': { src: kimi, mono: true },
};

export function dccName(dccType: string): string {
  return DCC_NAMES[dccType] ?? dccType;
}
