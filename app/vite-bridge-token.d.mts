import type { Plugin } from "vite";
declare const bridgeToken: (port: number) => Plugin;
export default bridgeToken;
export declare const devCsp: (port: number, bridgePort: number) => Plugin;
