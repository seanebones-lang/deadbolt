'use strict';
// Smithery's documented StdioDeployPayload supports full MCP tool schemas.
// MCPB 0.3 tools support names/descriptions only. Do not invent downstream tools.
// Source: https://smithery.ai/docs/api-reference/servers/publish-a-server
const manifest = require('./mcpb/manifest.json');
const { definitions } = require('./mcpb/server/setup.cjs');
if (JSON.stringify(manifest.tools) !== JSON.stringify(definitions.map(({ name, description }) => ({ name, description })))) {
  throw new Error('Setup manifest tools and runtime definitions differ');
}
const payload = {
  type: 'stdio', runtime: 'node',
  serverCard: {
    serverInfo: { name: manifest.name, version: manifest.version, description: manifest.description, websiteUrl: manifest.homepage },
    // These are the actual blank-configuration setup tools. Configured mode
    // replaces them with operator-selected downstream tools (tools_generated).
    tools: definitions,
  },
  configSchema: {
    type: 'object', properties: {
      operator_config: { type: 'string', title: manifest.user_config.operator_config.title, description: manifest.user_config.operator_config.description, default: '' },
    }, additionalProperties: false,
  },
};
process.stdout.write(JSON.stringify(payload, null, 2) + '\n');
