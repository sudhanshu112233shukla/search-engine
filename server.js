const config = require('./config');
const { createServer } = require('./src/api/server');

async function start() {
  const server = await createServer(config);
  server.listen(config.server.port, () => {
    console.log(`Search engine server listening on port ${config.server.port}`);
  });
}

start().catch((err) => {
  console.error('Failed to start server:', err);
  process.exit(1);
});
