import { defineConfig, type DevEnvironment, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

/**
 * vite-plugin-svelte answers a component's virtual CSS module (`X.svelte?svelte&type=style`)
 * from the module info of the compiled component. When the webview still holds the compiled
 * JS of a previous dev-server session, it asks for the CSS before the component has been
 * compiled in this session; the plugin then logs "failed to load virtual css module" and the
 * component renders unstyled until a reload. Compiling the component first closes the gap.
 */
function svelteCssAfterComponent(): Plugin {
  const virtualStyle = /[?&]svelte&type=style&lang\.css$/;
  return {
    name: "cartload:svelte-css-after-component",
    enforce: "pre",
    async load(id) {
      if (!virtualStyle.test(id) || !("transformRequest" in this.environment)) {
        return;
      }
      const filename = id.replace(/\?.*$/, "");
      if (this.getModuleInfo(filename)?.meta?.svelte?.css) {
        return;
      }
      const resolved = await this.resolve(filename);
      if (resolved) {
        // `this.load` does not record the plugin meta in the dev module graph; a transform
        // request through the dev server does.
        await (this.environment as DevEnvironment).transformRequest(resolved.id);
      }
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelteCssAfterComponent(), svelte()],
  // Tauri expects a fixed port and fails if it is not available.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // Do not watch the Rust side.
      ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"],
    },
  },
});
