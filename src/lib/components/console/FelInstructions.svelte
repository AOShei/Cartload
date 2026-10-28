<script lang="ts">
  import { onMount } from "svelte";
  import { inTauri } from "../../api/tauri";
  import { CONSOLE_MODELS, DEFAULT_MODEL, isKnownModel, modelFromProduct } from "../../models";
  import { consoleStore, preferences } from "../../state";
  import ConsoleTopView from "../ConsoleTopView.svelte";

  /* How to put the console in FEL mode, with a drawing of the model at hand. The drawing
     follows the console: the model is read from the USB product string while a stock
     console sits in normal boot, from hakchi once it runs, and can be picked by hand until
     then. The choice is remembered. `?model=` picks one in the browser preview. */
  interface Props {
    /** Drawing width. */
    width?: number;
    /** Show the model picker. */
    picker?: boolean;
  }

  let { width = 250, picker = true }: Props = $props();

  const status = $derived(consoleStore.report?.status ?? null);
  const previewModel = !inTauri && typeof location !== "undefined" ? new URLSearchParams(location.search).get("model") : null;

  $effect(() => {
    if (isKnownModel(previewModel)) {
      preferences.setConsoleModel(previewModel);
    }
  });

  /** The model the connected console reports, when it does. */
  const detectedModel = $derived.by((): string | null => {
    if (status?.state === "connected" && isKnownModel(status.info.console)) {
      return status.info.console;
    }
    if (status?.state === "fel") {
      return modelFromProduct(status.device.product);
    }
    return null;
  });

  /* The drawing switches to the detected model when shown and when the detection changes;
     in between the choice is free, as a fallback for a wrong guess. The detection is applied
     once per value, so the status refreshing every few seconds cannot undo a manual choice. */
  let appliedDetection: string | null = null;
  onMount(() => {
    if (detectedModel) {
      appliedDetection = detectedModel;
      preferences.setConsoleModel(detectedModel);
    }
  });
  $effect(() => {
    const detected = detectedModel;
    if (detected && detected !== appliedDetection) {
      appliedDetection = detected;
      preferences.setConsoleModel(detected);
    }
  });

  const consoleModel = $derived(isKnownModel(preferences.consoleModel) ? preferences.consoleModel : DEFAULT_MODEL);
</script>

<div class="fel">
  <div class="drawing">
    <ConsoleTopView model={consoleModel} {width} />
    {#if picker}
      <div class="model-pick">
        <select value={consoleModel} aria-label="Console model" onchange={(e) => preferences.setConsoleModel(e.currentTarget.value)}>
          {#each CONSOLE_MODELS as m (m.id)}
            <option value={m.id} selected={m.id === consoleModel}>{m.label}</option>
          {/each}
        </select>
        {#if detectedModel}
          <span class="muted detected">Detected: {CONSOLE_MODELS.find((m) => m.id === detectedModel)?.label ?? detectedModel}</span>
        {/if}
      </div>
    {/if}
  </div>
  <ol class="howto">
    <li>Connect the console to this computer with its USB cable and switch POWER off.</li>
    <li>Hold RESET down and switch POWER on.</li>
    <li>Keep RESET held for two seconds, then release it. The screen stays black: that is FEL mode.</li>
  </ol>
</div>

<style>
  .fel {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-5);
    align-items: center;
  }

  .drawing {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
  }

  .model-pick {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    font-size: var(--text-md);
  }

  .model-pick select {
    width: 250px;
    max-width: 100%;
  }

  .detected {
    font-size: var(--text-sm);
    text-align: center;
  }

  .howto {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  @media (max-width: 799px) {
    .fel {
      grid-template-columns: 1fr;
    }
  }
</style>
