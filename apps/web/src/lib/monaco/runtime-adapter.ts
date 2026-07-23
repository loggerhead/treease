import type { MonacoApi } from './public-types';

type ConfigurationService = {
  updateValue(key: string, value: unknown): void;
};

type StandaloneServicesModule = {
  StandaloneServices: {
    get(service: unknown): ConfigurationService;
  };
};

type ConfigurationModule = {
  IConfigurationService: unknown;
};

type MonacoJsonDefaults = {
  setDiagnosticsOptions(options: unknown): void;
};

type MonacoJsonContributionModule = {
  jsonDefaults?: MonacoJsonDefaults;
  default?: {
    jsonDefaults?: MonacoJsonDefaults;
  };
};

export async function loadMonacoApi(): Promise<MonacoApi> {
  return (await import('monaco-editor/esm/vs/editor/editor.api.js' as string)) as MonacoApi;
}

export async function loadMonacoStandaloneConfiguration(): Promise<{
  StandaloneServices: StandaloneServicesModule['StandaloneServices'];
  IConfigurationService: unknown;
}> {
  const [{ StandaloneServices }, { IConfigurationService }] = await Promise.all([
    import('monaco-editor/esm/vs/editor/standalone/browser/standaloneServices.js' as string) as Promise<StandaloneServicesModule>,
    import('monaco-editor/esm/vs/platform/configuration/common/configuration.js' as string) as Promise<ConfigurationModule>,
  ]);
  return { StandaloneServices, IConfigurationService };
}

export async function loadMonacoWorkers(): Promise<{
  editorWorkerCtor: { new (): Worker };
  jsonWorkerCtor: { new (): Worker };
}> {
  const [editorWorkerModule, jsonWorkerModule] = await Promise.all([
    import('monaco-editor/esm/vs/editor/editor.worker?worker'),
    import('monaco-editor/esm/vs/language/json/json.worker?worker'),
  ]);
  return {
    editorWorkerCtor: editorWorkerModule.default,
    jsonWorkerCtor: jsonWorkerModule.default,
  };
}

export async function loadMonacoJsonDefaults(): Promise<MonacoJsonDefaults> {
  const jsonModule = (await import(
    'monaco-editor/esm/vs/language/json/monaco.contribution.js' as string
  )) as MonacoJsonContributionModule;
  const jsonDefaults = jsonModule.jsonDefaults ?? jsonModule.default?.jsonDefaults;
  if (!jsonDefaults) {
    throw new Error('Monaco JSON contribution did not expose jsonDefaults');
  }
  return jsonDefaults;
}
