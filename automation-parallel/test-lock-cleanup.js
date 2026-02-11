/**
 * Comprehensive test suite for lockfile cleanup implementation
 * 
 * Tests:
 * 1. Verify startup cleanup is called in all run scripts
 * 2. Verify lockfile cleanup logic
 * 3. Simulate crash scenario
 * 4. Verify environment variable configuration
 * 5. Integration test with actual process
 */

const fs = require('fs').promises;
const fsSync = require('fs');
const path = require('path');
const os = require('os');
const StateManager = require('./lib/state-manager.js');

// Test result tracking
const testResults = {
  passed: [],
  failed: [],
  skipped: []
};

// Test temp directory
const TEST_STATE_DIR = path.join(__dirname, '.test-state');

/**
 * Helper: Create a test state directory
 */
async function setupTestDir() {
  try {
    await fs.rm(TEST_STATE_DIR, { recursive: true, force: true });
    await fs.mkdir(TEST_STATE_DIR, { recursive: true });
  } catch (e) {
    // Ignore errors
  }
}

/**
 * Helper: Clean up test directory
 */
async function cleanupTestDir() {
  try {
    await fs.rm(TEST_STATE_DIR, { recursive: true, force: true });
  } catch (e) {
    // Ignore errors
  }
}

/**
 * Helper: Create a lock file with specified data
 */
async function createLockFile(taskName, lockData) {
  const lockFilePath = path.join(TEST_STATE_DIR, `${taskName}.lock`);
  await fs.writeFile(lockFilePath, JSON.stringify(lockData, null, 2), 'utf8');
  return lockFilePath;
}

/**
 * Helper: Check if lock file exists
 */
function lockFileExists(taskName) {
  const lockFilePath = path.join(TEST_STATE_DIR, `${taskName}.lock`);
  return fsSync.existsSync(lockFilePath);
}

/**
 * Helper: Read lock file content
 */
async function readLockFile(taskName) {
  const lockFilePath = path.join(TEST_STATE_DIR, `${taskName}.lock`);
  const content = await fs.readFile(lockFilePath, 'utf8');
  return JSON.parse(content);
}

/**
 * Helper: Find a non-existent PID
 */
function findNonExistentPid() {
  // Use a very high PID that likely doesn't exist
  return 99999999;
}

/**
 * Test 1: Verify startup cleanup is called in all run scripts
 */
async function test1_startupCleanupCalled() {
  console.log('\n=== TEST 1: Verify startup cleanup is called in all run scripts ===');
  
  const runScripts = [
    { name: 'run-architect.js', path: path.join(__dirname, 'run-architect.js') },
    { name: 'run-janitor.js', path: path.join(__dirname, 'run-janitor.js') },
    { name: 'run-prompt.js', path: path.join(__dirname, 'run-prompt.js') }
  ];
  
  const results = [];
  
  for (const script of runScripts) {
    console.log(`\nChecking ${script.name}...`);
    const content = await fs.readFile(script.path, 'utf8');
    
    // Check for performStartupCleanup function
    const hasCleanupFunction = /async function performStartupCleanup\(\)/.test(content);
    console.log(`  - Has performStartupCleanup() function: ${hasCleanupFunction}`);
    
    // Check that it calls stateManager.cleanupStaleLocks()
    const callsCleanup = /await stateManager\.cleanupStaleLocks\(\)/.test(content);
    console.log(`  - Calls stateManager.cleanupStaleLocks(): ${callsCleanup}`);
    
    // Check that it's called before task.start()
    const cleanupBeforeStart = /await performStartupCleanup\(\);[\s\S]*task\.start\(\)/.test(content);
    console.log(`  - Called before task.start(): ${cleanupBeforeStart}`);
    
    const allChecksPassed = hasCleanupFunction && callsCleanup && cleanupBeforeStart;
    results.push({ script: script.name, passed: allChecksPassed });
  }
  
  const allPassed = results.every(r => r.passed);
  
  if (allPassed) {
    console.log('\n✅ TEST 1 PASSED: All run scripts have startup cleanup properly implemented');
    testResults.passed.push('Test 1: Startup cleanup is called');
  } else {
    console.log('\n❌ TEST 1 FAILED: Some run scripts are missing proper startup cleanup');
    testResults.failed.push('Test 1: Startup cleanup is called');
    results.forEach(r => {
      if (!r.passed) console.log(`  - ${r.script}: FAILED`);
    });
  }
  
  return allPassed;
}

/**
 * Test 2a: Verify _isProcessRunning correctly identifies running vs dead processes
 */
async function test2a_processRunningCheck() {
  console.log('\n=== TEST 2a: Verify _isProcessRunning() correctly identifies running vs dead processes ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  const tests = [];
  
  // Test with current process (should be running)
  try {
    const isRunning = await stateManager._isProcessRunning(process.pid);
    console.log(`\n  Current PID (${process.pid}) is running: ${isRunning}`);
    tests.push({ name: 'Current process should be running', expected: true, actual: isRunning });
  } catch (e) {
    console.log(`  Error checking current PID: ${e.message}`);
    tests.push({ name: 'Current process should be running', expected: true, actual: false, error: e.message });
  }
  
  // Test with non-existent PID (should not be running)
  const nonExistentPid = findNonExistentPid();
  const isDeadRunning = await stateManager._isProcessRunning(nonExistentPid);
  console.log(`  Non-existent PID (${nonExistentPid}) is running: ${isDeadRunning}`);
  tests.push({ name: 'Non-existent process should not be running', expected: false, actual: isDeadRunning });
  
  // Test with PID 1 (usually exists on Unix systems)
  try {
    const pid1Running = await stateManager._isProcessRunning(1);
    console.log(`  PID 1 is running: ${pid1Running}`);
    tests.push({ name: 'PID 1 check', expected: true, actual: pid1Running });
  } catch (e) {
    // On Windows, PID 1 might not be accessible
    console.log(`  PID 1 check: ${e.message} (expected on Windows)`);
    tests.push({ name: 'PID 1 check', expected: true, actual: false, skipped: true });
  }
  
  const allPassed = tests.filter(t => !t.skipped).every(t => t.expected === t.actual);
  
  if (allPassed) {
    console.log('\n✅ TEST 2a PASSED: _isProcessRunning() correctly identifies process status');
    testResults.passed.push('Test 2a: _isProcessRunning() correctly identifies process status');
  } else {
    console.log('\n❌ TEST 2a FAILED: _isProcessRunning() has issues');
    tests.forEach(t => {
      if (t.skipped) {
        console.log(`  - ${t.name}: SKIPPED (${t.error})`);
      } else if (t.expected !== t.actual) {
        console.log(`  - ${t.name}: FAILED (expected ${t.expected}, got ${t.actual})`);
      }
    });
    testResults.failed.push('Test 2a: _isProcessRunning() correctly identifies process status');
  }
  
  return allPassed;
}

/**
 * Test 2b: Verify cleanupStaleLocks() cleans locks from dead processes
 */
async function test2b_cleanupDeadProcessLocks() {
  console.log('\n=== TEST 2b: Verify cleanupStaleLocks() cleans locks from dead processes ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create lock files with dead processes
  const nonExistentPid = findNonExistentPid();
  await createLockFile('dead-task', {
    pid: nonExistentPid,
    timestamp: Date.now() - 10000, // 10 seconds ago
    host: os.hostname()
  });
  
  await createLockFile('another-dead-task', {
    pid: nonExistentPid + 1,
    timestamp: Date.now() - 5000, // 5 seconds ago
    host: os.hostname()
  });
  
  // Create a lock with a running process (this one should remain)
  await createLockFile('live-task', {
    pid: process.pid,
    timestamp: Date.now() - 1000, // 1 second ago
    host: os.hostname()
  });
  
  console.log(`\n  Created 3 lock files:`);
  console.log(`    - dead-task: PID ${nonExistentPid} (dead)`);
  console.log(`    - another-dead-task: PID ${nonExistentPid + 1} (dead)`);
  console.log(`    - live-task: PID ${process.pid} (running)`);
  
  // Run cleanup
  const result = await stateManager.cleanupStaleLocks();
  
  console.log(`\n  Cleanup result: ${result.cleanedCount} locks cleaned`);
  result.cleanedDetails.forEach(detail => {
    console.log(`    - ${detail.taskName}: ${detail.reason}`);
  });
  
  // Verify results
  const deadTaskExists = lockFileExists('dead-task');
  const anotherDeadTaskExists = lockFileExists('another-dead-task');
  const liveTaskExists = lockFileExists('live-task');
  
  console.log(`\n  After cleanup:`);
  console.log(`    - dead-task exists: ${deadTaskExists}`);
  console.log(`    - another-dead-task exists: ${anotherDeadTaskExists}`);
  console.log(`    - live-task exists: ${liveTaskExists}`);
  
  await cleanupTestDir();
  
  const allPassed = !deadTaskExists && !anotherDeadTaskExists && liveTaskExists;
  
  if (allPassed) {
    console.log('\n✅ TEST 2b PASSED: cleanupStaleLocks() correctly cleans locks from dead processes');
    testResults.passed.push('Test 2b: cleanupStaleLocks() cleans dead process locks');
  } else {
    console.log('\n❌ TEST 2b FAILED: cleanupStaleLocks() did not clean properly');
    testResults.failed.push('Test 2b: cleanupStaleLocks() cleans dead process locks');
  }
  
  return allPassed;
}

/**
 * Test 2c: Verify cleanupStaleLocks() cleans locks older than threshold
 */
async function test2b_cleanupOldLocks() {
  console.log('\n=== TEST 2c: Verify cleanupStaleLocks() cleans locks older than threshold ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  const maxAgeMs = 5000; // 5 seconds threshold
  
  // Create an old lock (older than threshold)
  await createLockFile('old-task', {
    pid: process.pid,
    timestamp: Date.now() - 10000, // 10 seconds ago (older than threshold)
    host: os.hostname()
  });
  
  // Create a new lock (younger than threshold)
  await createLockFile('new-task', {
    pid: process.pid,
    timestamp: Date.now() - 1000, // 1 second ago (younger than threshold)
    host: os.hostname()
  });
  
  console.log(`\n  Created 2 lock files with threshold ${maxAgeMs}ms:`);
  console.log(`    - old-task: 10 seconds old`);
  console.log(`    - new-task: 1 second old`);
  
  // Run cleanup with custom threshold
  const result = await stateManager.cleanupStaleLocks(maxAgeMs);
  
  console.log(`\n  Cleanup result: ${result.cleanedCount} locks cleaned`);
  result.cleanedDetails.forEach(detail => {
    console.log(`    - ${detail.taskName}: ${detail.reason}`);
  });
  
  // Verify results
  const oldTaskExists = lockFileExists('old-task');
  const newTaskExists = lockFileExists('new-task');
  
  console.log(`\n  After cleanup:`);
  console.log(`    - old-task exists: ${oldTaskExists}`);
  console.log(`    - new-task exists: ${newTaskExists}`);
  
  await cleanupTestDir();
  
  const allPassed = !oldTaskExists && newTaskExists;
  
  if (allPassed) {
    console.log('\n✅ TEST 2c PASSED: cleanupStaleLocks() correctly cleans locks older than threshold');
    testResults.passed.push('Test 2c: cleanupStaleLocks() cleans old locks');
  } else {
    console.log('\n❌ TEST 2c FAILED: cleanupStaleLocks() did not clean old locks properly');
    testResults.failed.push('Test 2c: cleanupStaleLocks() cleans old locks');
  }
  
  return allPassed;
}

/**
 * Test 2d: Verify cleanupStaleLocks() keeps locks from running processes
 */
async function test2c_keepsRunningProcessLocks() {
  console.log('\n=== TEST 2d: Verify cleanupStaleLocks() keeps locks from running processes ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create lock with running process
  await createLockFile('running-task', {
    pid: process.pid,
    timestamp: Date.now() - 1000,
    host: os.hostname()
  });
  
  console.log(`\n  Created lock file with running process (PID ${process.pid})`);
  
  // Run cleanup
  const result = await stateManager.cleanupStaleLocks();
  
  console.log(`\n  Cleanup result: ${result.cleanedCount} locks cleaned`);
  
  // Verify lock still exists
  const lockExists = lockFileExists('running-task');
  console.log(`\n  Lock exists after cleanup: ${lockExists}`);
  
  await cleanupTestDir();
  
  if (lockExists) {
    console.log('\n✅ TEST 2d PASSED: cleanupStaleLocks() keeps locks from running processes');
    testResults.passed.push('Test 2d: cleanupStaleLocks() keeps running process locks');
  } else {
    console.log('\n❌ TEST 2d FAILED: cleanupStaleLocks() incorrectly removed running process lock');
    testResults.failed.push('Test 2d: cleanupStaleLocks() keeps running process locks');
  }
  
  return lockExists;
}

/**
 * Test 2e: Verify cleanupStaleLocks() handles cross-host locks
 */
async function test2d_crossHostLocks() {
  console.log('\n=== TEST 2e: Verify cleanupStaleLocks() handles cross-host locks ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create lock from different host (old enough to expire)
  await createLockFile('cross-host-old', {
    pid: 12345,
    timestamp: Date.now() - 70000, // 70 seconds ago (older than 1 min cross-host threshold)
    host: 'different-host.example.com'
  });
  
  // Create lock from different host (recent - should remain)
  await createLockFile('cross-host-new', {
    pid: 12346,
    timestamp: Date.now() - 30000, // 30 seconds ago (newer than 1 min threshold)
    host: 'different-host.example.com'
  });
  
  console.log(`\n  Created 2 cross-host lock files:`);
  console.log(`    - cross-host-old: 70 seconds old`);
  console.log(`    - cross-host-new: 30 seconds old`);
  console.log(`    Current hostname: ${os.hostname()}`);
  
  // Run cleanup
  const result = await stateManager.cleanupStaleLocks();
  
  console.log(`\n  Cleanup result: ${result.cleanedCount} locks cleaned`);
  result.cleanedDetails.forEach(detail => {
    console.log(`    - ${detail.taskName}: ${detail.reason}`);
  });
  
  // Verify results
  const oldCrossHostExists = lockFileExists('cross-host-old');
  const newCrossHostExists = lockFileExists('cross-host-new');
  
  console.log(`\n  After cleanup:`);
  console.log(`    - cross-host-old exists: ${oldCrossHostExists}`);
  console.log(`    - cross-host-new exists: ${newCrossHostExists}`);
  
  await cleanupTestDir();
  
  const allPassed = !oldCrossHostExists && newCrossHostExists;
  
  if (allPassed) {
    console.log('\n✅ TEST 2e PASSED: cleanupStaleLocks() correctly handles cross-host locks');
    testResults.passed.push('Test 2e: cleanupStaleLocks() handles cross-host locks');
  } else {
    console.log('\n❌ TEST 2e FAILED: cleanupStaleLocks() did not handle cross-host locks properly');
    testResults.failed.push('Test 2e: cleanupStaleLocks() handles cross-host locks');
  }
  
  return allPassed;
}

/**
 * Test 2f: Verify cleanupStaleLocks() handles same-PID restarts
 */
async function test2e_samePidRestarts() {
  console.log('\n=== TEST 2f: Verify cleanupStaleLocks() handles same-PID restarts ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create lock with current PID (simulating stale lock from restart)
  await createLockFile('same-pid-task', {
    pid: process.pid,
    timestamp: Date.now() - 10000,
    host: os.hostname()
  });
  
  console.log(`\n  Created lock file with current PID ${process.pid} (stale from restart)`);
  
  // Run cleanup
  const result = await stateManager.cleanupStaleLocks();
  
  console.log(`\n  Cleanup result: ${result.cleanedCount} locks cleaned`);
  result.cleanedDetails.forEach(detail => {
    console.log(`    - ${detail.taskName}: ${detail.reason}`);
  });
  
  // Verify lock was cleaned
  const lockExists = lockFileExists('same-pid-task');
  console.log(`\n  Lock exists after cleanup: ${lockExists}`);
  
  await cleanupTestDir();
  
  if (!lockExists) {
    console.log('\n✅ TEST 2f PASSED: cleanupStaleLocks() correctly handles same-PID restarts');
    testResults.passed.push('Test 2f: cleanupStaleLocks() handles same-PID restarts');
  } else {
    console.log('\n❌ TEST 2f FAILED: cleanupStaleLocks() did not clean same-PID lock');
    testResults.failed.push('Test 2f: cleanupStaleLocks() handles same-PID restarts');
  }
  
  return !lockExists;
}

/**
 * Test 2g: Verify cleanupStaleLocks() handles corrupted lockfiles
 */
async function test2f_corruptedLockfiles() {
  console.log('\n=== TEST 2g: Verify cleanupStaleLocks() handles corrupted lockfiles ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create corrupted lockfile (invalid JSON)
  const corruptedLockPath = path.join(TEST_STATE_DIR, 'corrupted.lock');
  await fs.writeFile(corruptedLockPath, 'invalid json content {{{', 'utf8');
  
  // Create lockfile without required fields
  const incompleteLockPath = path.join(TEST_STATE_DIR, 'incomplete.lock');
  await fs.writeFile(incompleteLockPath, JSON.stringify({ foo: 'bar' }), 'utf8');
  
  // Create valid lockfile
  await createLockFile('valid-task', {
    pid: process.pid,
    timestamp: Date.now() - 1000,
    host: os.hostname()
  });
  
  console.log(`\n  Created 3 lock files:`);
  console.log(`    - corrupted.lock: invalid JSON`);
  console.log(`    - incomplete.lock: missing required fields`);
  console.log(`    - valid-task: valid lockfile`);
  
  // Run cleanup
  const result = await stateManager.cleanupStaleLocks();
  
  console.log(`\n  Cleanup result: ${result.cleanedCount} locks cleaned`);
  result.cleanedDetails.forEach(detail => {
    console.log(`    - ${detail.taskName}: ${detail.reason}`);
  });
  
  // Verify results
  const corruptedExists = fsSync.existsSync(corruptedLockPath);
  const incompleteExists = fsSync.existsSync(incompleteLockPath);
  const validExists = lockFileExists('valid-task');
  
  console.log(`\n  After cleanup:`);
  console.log(`    - corrupted.lock exists: ${corruptedExists}`);
  console.log(`    - incomplete.lock exists: ${incompleteExists}`);
  console.log(`    - valid-task exists: ${validExists}`);
  
  await cleanupTestDir();
  
  const allPassed = !corruptedExists && !incompleteExists && validExists;
  
  if (allPassed) {
    console.log('\n✅ TEST 2g PASSED: cleanupStaleLocks() correctly handles corrupted lockfiles');
    testResults.passed.push('Test 2g: cleanupStaleLocks() handles corrupted lockfiles');
  } else {
    console.log('\n❌ TEST 2g FAILED: cleanupStaleLocks() did not handle corrupted lockfiles properly');
    testResults.failed.push('Test 2g: cleanupStaleLocks() handles corrupted lockfiles');
  }
  
  return allPassed;
}

/**
 * Test 3: Simulate crash scenario
 */
async function test3_crashScenario() {
  console.log('\n=== TEST 3: Simulate crash scenario ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create a stale lockfile (simulating a crashed container)
  const nonExistentPid = findNonExistentPid();
  await createLockFile('crashed-task', {
    pid: nonExistentPid,
    timestamp: Date.now() - 30000, // 30 seconds ago
    host: os.hostname()
  });
  
  console.log(`\n  Simulating crash: created stale lock for crashed-task`);
  console.log(`    - PID: ${nonExistentPid} (non-existent)`);
  console.log(`    - Age: 30 seconds`);
  console.log(`    - Host: ${os.hostname()}`);
  
  // Simulate container startup by running cleanup
  console.log(`\n  Simulating container startup...`);
  const cleanupResult = await stateManager.cleanupStaleLocks();
  
  console.log(`\n  Cleanup result: ${cleanupResult.cleanedCount} locks cleaned`);
  if (cleanupResult.cleanedDetails.length > 0) {
    cleanupResult.cleanedDetails.forEach(detail => {
      console.log(`    - ${detail.taskName}: ${detail.reason} (pid: ${detail.pid}, host: ${detail.hostname})`);
    });
  }
  
  // Verify the lock was cleaned
  const lockExists = lockFileExists('crashed-task');
  console.log(`\n  Lock exists after cleanup: ${lockExists}`);
  
  // Try to acquire a new lock (simulating container starting work)
  let lockAcquired = false;
  try {
    await stateManager.acquireLock('crashed-task');
    lockAcquired = true;
    console.log(`\n  ✅ New lock acquired successfully after cleanup`);
    await stateManager.releaseLock('crashed-task');
  } catch (e) {
    console.log(`\n  ❌ Failed to acquire lock: ${e.message}`);
  }
  
  await cleanupTestDir();
  
  const testPassed = !lockExists && lockAcquired;
  
  if (testPassed) {
    console.log('\n✅ TEST 3 PASSED: Container can start properly after crash');
    testResults.passed.push('Test 3: Crash scenario - container starts after crash');
  } else {
    console.log('\n❌ TEST 3 FAILED: Container cannot start properly after crash');
    testResults.failed.push('Test 3: Crash scenario - container starts after crash');
  }
  
  return testPassed;
}

/**
 * Test 4: Verify environment variable configuration
 */
async function test4_environmentVariableConfig() {
  console.log('\n=== TEST 4: Verify environment variable configuration (LOCK_CLEANUP_MAX_AGE_MS) ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Test 4a: Default value (should be 5 minutes = 300000ms)
  delete process.env.LOCK_CLEANUP_MAX_AGE_MS;
  const defaultMaxAge = stateManager._getLockCleanupMaxAge();
  console.log(`\n  Test 4a: Default max age`);
  console.log(`    Expected: 300000 (5 minutes)`);
  console.log(`    Actual: ${defaultMaxAge}`);
  const test4aPassed = defaultMaxAge === 300000;
  
  // Test 4b: Custom value from environment variable
  process.env.LOCK_CLEANUP_MAX_AGE_MS = '60000'; // 1 minute
  const customMaxAge = stateManager._getLockCleanupMaxAge();
  console.log(`\n  Test 4b: Custom max age from env var`);
  console.log(`    LOCK_CLEANUP_MAX_AGE_MS: 60000`);
  console.log(`    Actual: ${customMaxAge}`);
  const test4bPassed = customMaxAge === 60000;
  
  // Test 4c: Invalid value should use default
  process.env.LOCK_CLEANUP_MAX_AGE_MS = 'invalid';
  const invalidMaxAge = stateManager._getLockCleanupMaxAge();
  console.log(`\n  Test 4c: Invalid env var value`);
  console.log(`    LOCK_CLEANUP_MAX_AGE_MS: invalid`);
  console.log(`    Expected: 300000 (default)`);
  console.log(`    Actual: ${invalidMaxAge}`);
  const test4cPassed = invalidMaxAge === 300000;
  
  // Test 4d: Verify cleanupStaleLocks() respects the env var
  process.env.LOCK_CLEANUP_MAX_AGE_MS = '2000'; // 2 seconds
  await createLockFile('old-lock', {
    pid: process.pid,
    timestamp: Date.now() - 3000, // 3 seconds ago (older than 2s threshold)
    host: os.hostname()
  });
  await createLockFile('new-lock', {
    pid: process.pid,
    timestamp: Date.now() - 1000, // 1 second ago (younger than 2s threshold)
    host: os.hostname()
  });
  
  console.log(`\n  Test 4d: Verify cleanup respects env var (threshold: 2s)`);
  console.log(`    - old-lock: 3 seconds old`);
  console.log(`    - new-lock: 1 second old`);
  
  const cleanupResult = await stateManager.cleanupStaleLocks();
  const oldExists = lockFileExists('old-lock');
  const newExists = lockFileExists('new-lock');
  
  console.log(`\n  After cleanup:`);
  console.log(`    - old-lock exists: ${oldExists}`);
  console.log(`    - new-lock exists: ${newExists}`);
  
  const test4dPassed = !oldExists && newExists;
  
  // Clean up
  delete process.env.LOCK_CLEANUP_MAX_AGE_MS;
  await cleanupTestDir();
  
  const allPassed = test4aPassed && test4bPassed && test4cPassed && test4dPassed;
  
  if (allPassed) {
    console.log('\n✅ TEST 4 PASSED: Environment variable configuration works correctly');
    testResults.passed.push('Test 4: Environment variable configuration');
  } else {
    console.log('\n❌ TEST 4 FAILED: Environment variable configuration has issues');
    if (!test4aPassed) console.log('  - Test 4a failed: Default value incorrect');
    if (!test4bPassed) console.log('  - Test 4b failed: Custom value not respected');
    if (!test4cPassed) console.log('  - Test 4c failed: Invalid value handling incorrect');
    if (!test4dPassed) console.log('  - Test 4d failed: Cleanup does not respect env var');
    testResults.failed.push('Test 4: Environment variable configuration');
  }
  
  return allPassed;
}

/**
 * Test 5: Integration test with actual process
 */
async function test5_integrationTest() {
  console.log('\n=== TEST 5: Integration test with actual process ===');
  
  const stateManager = new StateManager(TEST_STATE_DIR);
  await setupTestDir();
  
  // Create a child process to simulate a container
  const { spawn } = require('child_process');
  
  console.log(`\n  Step 1: Creating child process to simulate container...`);
  const childProcess = spawn('node', ['-e', 'setInterval(() => {}, 1000);'], {
    stdio: 'ignore'
  });
  
  console.log(`    Child PID: ${childProcess.pid}`);
  
  // Give process time to start
  await new Promise(resolve => setTimeout(resolve, 100));
  
  // Create lock file for the child process
  await createLockFile('integration-task', {
    pid: childProcess.pid,
    timestamp: Date.now() - 1000,
    host: os.hostname()
  });
  
  console.log(`    Created lock for integration-task`);
  
  // Test 5a: Verify lock is kept for running process
  console.log(`\n  Step 2: Running cleanup (should keep lock for running process)...`);
  let result = await stateManager.cleanupStaleLocks();
  let lockExists = lockFileExists('integration-task');
  console.log(`    Lock exists: ${lockExists}`);
  
  const test5aPassed = lockExists;
  
  // Test 5b: Kill the process and verify lock is cleaned
  console.log(`\n  Step 3: Simulating crash (killing child process)...`);
  childProcess.kill();
  
  // Give OS time to clean up the process
  await new Promise(resolve => setTimeout(resolve, 200));
  
  console.log(`\n  Step 4: Running cleanup (should clean lock for dead process)...`);
  result = await stateManager.cleanupStaleLocks();
  lockExists = lockFileExists('integration-task');
  console.log(`    Lock exists: ${lockExists}`);
  console.log(`    Cleaned locks: ${result.cleanedCount}`);
  
  const test5bPassed = !lockExists;
  
  // Test 5c: Verify we can now acquire the lock
  console.log(`\n  Step 5: Attempting to acquire lock...`);
  let lockAcquired = false;
  try {
    await stateManager.acquireLock('integration-task');
    lockAcquired = true;
    console.log(`    ✅ Lock acquired successfully`);
    await stateManager.releaseLock('integration-task');
  } catch (e) {
    console.log(`    ❌ Failed to acquire lock: ${e.message}`);
  }
  
  const test5cPassed = lockAcquired;
  
  await cleanupTestDir();
  
  const allPassed = test5aPassed && test5bPassed && test5cPassed;
  
  if (allPassed) {
    console.log('\n✅ TEST 5 PASSED: Integration test with actual process successful');
    testResults.passed.push('Test 5: Integration test');
  } else {
    console.log('\n❌ TEST 5 FAILED: Integration test failed');
    if (!test5aPassed) console.log('  - Test 5a failed: Lock not kept for running process');
    if (!test5bPassed) console.log('  - Test 5b failed: Lock not cleaned after process death');
    if (!test5cPassed) console.log('  - Test 5c failed: Cannot acquire lock after cleanup');
    testResults.failed.push('Test 5: Integration test');
  }
  
  return allPassed;
}

/**
 * Main test runner
 */
async function runAllTests() {
  console.log('╔════════════════════════════════════════════════════════════════╗');
  console.log('║     LOCKFILE CLEANUP IMPLEMENTATION TEST SUITE               ║');
  console.log('╚════════════════════════════════════════════════════════════════╝');
  console.log(`\nTest directory: ${TEST_STATE_DIR}`);
  console.log(`Current PID: ${process.pid}`);
  console.log(`Hostname: ${os.hostname()}`);
  console.log(`Platform: ${process.platform}`);
  console.log(`Node version: ${process.version}`);
  
  await setupTestDir();
  
  try {
    // Test 1
    await test1_startupCleanupCalled();
    
    // Test 2 series
    await test2a_processRunningCheck();
    await test2b_cleanupDeadProcessLocks();
    await test2b_cleanupOldLocks();
    await test2c_keepsRunningProcessLocks();
    await test2d_crossHostLocks();
    await test2e_samePidRestarts();
    await test2f_corruptedLockfiles();
    
    // Test 3
    await test3_crashScenario();
    
    // Test 4
    await test4_environmentVariableConfig();
    
    // Test 5
    await test5_integrationTest();
    
  } catch (error) {
    console.error(`\n❌ Fatal error during tests: ${error.message}`);
    console.error(error.stack);
  } finally {
    await cleanupTestDir();
  }
  
  // Print summary
  console.log('\n╔════════════════════════════════════════════════════════════════╗');
  console.log('║                        TEST SUMMARY                            ║');
  console.log('╚════════════════════════════════════════════════════════════════╝');
  
  console.log(`\nTotal tests run: ${testResults.passed.length + testResults.failed.length}`);
  console.log(`✅ Passed: ${testResults.passed.length}`);
  console.log(`❌ Failed: ${testResults.failed.length}`);
  
  if (testResults.passed.length > 0) {
    console.log('\nPassed tests:');
    testResults.passed.forEach(test => console.log(`  ✓ ${test}`));
  }
  
  if (testResults.failed.length > 0) {
    console.log('\nFailed tests:');
    testResults.failed.forEach(test => console.log(`  ✗ ${test}`));
  }
  
  const allPassed = testResults.failed.length === 0;
  
  if (allPassed) {
    console.log('\n╔════════════════════════════════════════════════════════════════╗');
    console.log('║  ✅ ALL TESTS PASSED - Lockfile cleanup implementation works! ║');
    console.log('╚════════════════════════════════════════════════════════════════╝');
  } else {
    console.log('\n╔════════════════════════════════════════════════════════════════╗');
    console.log('║  ⚠️  SOME TESTS FAILED - Review implementation issues             ║');
    console.log('╚════════════════════════════════════════════════════════════════╝');
  }
  
  return allPassed;
}

// Run tests if this file is executed directly
if (require.main === module) {
  runAllTests()
    .then(success => {
      process.exit(success ? 0 : 1);
    })
    .catch(error => {
      console.error('Test runner error:', error);
      process.exit(1);
    });
}

module.exports = { runAllTests };
