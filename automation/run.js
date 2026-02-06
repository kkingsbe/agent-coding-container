const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Configuration
const INTERVAL = 0;//(process.argv[2] || 600) * 1000;
const LOOP_TYPE = process.argv[2] || 'development'; // 'development', 'bugfixer', or 'linter'

// Development loop prompts
const PROMPT_PATH = path.join(__dirname, 'prompts/development/PROMPT.md');
const JANITOR_PATH = path.join(__dirname, 'prompts/development/JANITOR.md');
const ARCHITECT_PATH = path.join(__dirname, 'prompts/development/ARCHITECT.md');

// Bugfixer loop prompts
const BUGFIXER_PATH = path.join(__dirname, 'prompts/bugfixer/BUGFIXER.md');
const BUGFIXER_BUGCHECK_PATH = path.join(__dirname, 'prompts/bugfixer/BUGFIXER_BUGCHECK.md');

// Linter loop prompts
const LINTER_PATH = path.join(__dirname, 'prompts/linter/LINTER.md');
const LINTER_SCAN_PATH = path.join(__dirname, 'prompts/linter/LINTER_SCAN.md');
const LINTER_PRIORITIZE_PATH = path.join(__dirname, 'prompts/linter/LINTER_PRIORITIZE.md');

const DONE_FILE = '.done';

const WORKSPACE_PATH = path.resolve(__dirname, '../workspace');
const STATE_FILE = path.join(WORKSPACE_PATH, `.state_${LOOP_TYPE}.json`);

// Continuation markers - prompts that support idempotent resumption
const CONTINUATION_CONFIG = {
    'ARCHITECT': {
        marker: '.architect_in_progress',
        stateFile: 'ARCHITECT_STATE.md'
    },
    'LINTER_SCAN': {
        marker: '.linter_scan_in_progress',
        stateFile: 'LINTER_SCAN_STATE.md'
    },
    'LINTER_PRIORITIZE': {
        marker: '.linter_prioritize_in_progress',
        stateFile: 'LINTER_PRIORITIZE_STATE.md'
    },
    'BUGFIXER_BUGCHECK': {
        marker: '.bugfixer_bugcheck_in_progress',
        stateFile: 'BUGFIXER_BUGCHECK_STATE.md'
    }
};

// Prompt runner mapping
const PROMPT_RUNNERS = {
    'PROMPT': () => runKiloWithPrompt(PROMPT_PATH, 'PROMPT'),
    'JANITOR': () => runKiloWithPrompt(JANITOR_PATH, 'JANITOR'),
    'ARCHITECT': () => runKiloWithPrompt(ARCHITECT_PATH, 'ARCHITECT'),
    'BUGFIXER': () => runKiloWithPrompt(BUGFIXER_PATH, 'BUGFIXER'),
    'BUGFIXER_BUGCHECK': () => runKiloWithPrompt(BUGFIXER_BUGCHECK_PATH, 'BUGFIXER_BUGCHECK'),
    'LINTER': () => runKiloWithPrompt(LINTER_PATH, 'LINTER'),
    'LINTER_SCAN': () => runKiloWithPrompt(LINTER_SCAN_PATH, 'LINTER_SCAN'),
    'LINTER_PRIORITIZE': () => runKiloWithPrompt(LINTER_PRIORITIZE_PATH, 'LINTER_PRIORITIZE')
};

// State management functions
function loadState() {
    try {
        if (fs.existsSync(STATE_FILE)) {
            const stateContent = fs.readFileSync(STATE_FILE, 'utf8');
            const state = JSON.parse(stateContent);
            console.log(`📂 Loaded state from ${STATE_FILE}`);
            console.log(`   - iteration: ${state.iteration}`);
            console.log(`   - lastRun: ${state.lastRun}`);
            return state;
        }
        console.log(`ℹ️ No state file found at ${STATE_FILE}, starting fresh`);
        return null;
    } catch (error) {
        console.warn(`⚠️ Warning: Failed to load state file: ${error.message}`);
        console.warn(`   Starting from iteration 1`);
        return null;
    }
}

function saveState(iteration) {
    try {
        const state = {
            iteration: iteration,
            lastRun: new Date().toISOString()
        };
        fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2), 'utf8');
        console.log(`💾 Saved state to ${STATE_FILE}`);
        console.log(`   - iteration: ${state.iteration}`);
        console.log(`   - lastRun: ${state.lastRun}`);
    } catch (error) {
        console.error(`❌ Error: Failed to save state file: ${error.message}`);
    }
}

// Continuation detection - check for incomplete prompts from previous runs
function checkForContinuation() {
    for (const [promptName, config] of Object.entries(CONTINUATION_CONFIG)) {
        const markerPath = path.join(WORKSPACE_PATH, config.marker);
        if (fs.existsSync(markerPath)) {
            const stateFilePath = path.join(WORKSPACE_PATH, config.stateFile);
            const hasStateFile = fs.existsSync(stateFilePath);
            console.log(`🔄 Found incomplete ${promptName} session`);
            console.log(`   - Marker: ${config.marker}`);
            console.log(`   - State file exists: ${hasStateFile}`);
            if (hasStateFile) {
                try {
                    const stateContent = fs.readFileSync(stateFilePath, 'utf8');
                    // Extract status line if present
                    const statusMatch = stateContent.match(/Status:\s*(\w+)/i);
                    if (statusMatch) {
                        console.log(`   - Status: ${statusMatch[1]}`);
                    }
                } catch (e) {
                    // Ignore read errors
                }
            }
            return promptName;
        }
    }
    return null;
}

// Reusable function to run kilocode with any prompt
function runKiloWithPrompt(promptPath, promptName) {
    console.log(`${new Date().toLocaleString()}: Starting ${promptName}...`);
    console.log(`Target Workspace: ${WORKSPACE_PATH}`);

    const promptContent = fs.readFileSync(promptPath, 'utf8').trim();

    const args = [
        '--mode', 'orchestrator',
        '--auto',
        '--timeout', '900',
        '--workspace', WORKSPACE_PATH
    ];

    console.log(`\nRunning 'kilocode' with piped ${promptName} (${promptContent.length} chars)...\n`);

    const result = spawnSync('kilocode', args, {
        stdio: ['pipe', 'inherit', 'inherit'],
        input: promptContent,
        shell: true
    });

    if (result.error) {
        console.error(`❌ ${promptName} execution failed:`, result.error.message);
        return -1;
    }

    console.log(`${new Date().toLocaleString()}: ${promptName} completed with exit code ${result.status}`);
    return result.status;
}

async function main() {
    // 0. Validate loop type
    const validLoopTypes = ['development', 'bugfixer', 'linter'];
    if (!validLoopTypes.includes(LOOP_TYPE)) {
        console.error(`❌ Error: Invalid loop type '${LOOP_TYPE}'. Valid options: ${validLoopTypes.join(', ')}`);
        process.exit(1);
    }
    console.log(`🔄 Running in '${LOOP_TYPE}' loop mode`);

    // 1. Check Prompt existence based on loop type
    let requiredPrompts;
    if (LOOP_TYPE === 'development') {
        requiredPrompts = [PROMPT_PATH, JANITOR_PATH, ARCHITECT_PATH];
    } else if (LOOP_TYPE === 'bugfixer') {
        requiredPrompts = [BUGFIXER_PATH, BUGFIXER_BUGCHECK_PATH];
    } else if (LOOP_TYPE === 'linter') {
        requiredPrompts = [LINTER_PATH, LINTER_SCAN_PATH, LINTER_PRIORITIZE_PATH];
    }

    for (const promptPath of requiredPrompts) {
        if (!fs.existsSync(promptPath)) {
            console.error(`❌ Error: Prompt file not found at ${promptPath}`);
            process.exit(1);
        }
    }

    // 2. Check Workspace existence (Sanity check)
    if (!fs.existsSync(WORKSPACE_PATH)) {
        console.error(`❌ Error: Workspace folder not found at ${WORKSPACE_PATH}`);
        process.exit(1);
    }

    // 3. Load saved state or start fresh
    const savedState = loadState();
    let iteration = savedState ? savedState.iteration + 1 : 1;
    console.log(`🚀 Starting ${LOOP_TYPE} automation from iteration #${iteration}`);

    while (true) {
        console.log(`\n${'='.repeat(60)}`);
        console.log(`${new Date().toLocaleString()}: Iteration #${iteration}`);
        console.log(`${'='.repeat(60)}\n`);

        // Build queue of prompts to run for this iteration
        const promptQueue = [];

        // FIRST: Check for any incomplete prompts that need continuation
        const continuationPrompt = checkForContinuation();
        
        if (continuationPrompt) {
            // Priority: Resume incomplete work before normal scheduling
            console.log(`📋 Prioritizing continuation of ${continuationPrompt}`);
            promptQueue.push({ 
                name: continuationPrompt, 
                fn: PROMPT_RUNNERS[continuationPrompt],
                isContinuation: true 
            });
        }

        // THEN: Add normally scheduled prompts (if not already queued for continuation)
        if (LOOP_TYPE === 'development') {
            // Development loop: PROMPT + JANITOR (every 4) + ARCHITECT (every 8)
            promptQueue.push({ name: 'PROMPT', fn: PROMPT_RUNNERS['PROMPT'] });

            // Check for JANITOR.md (every 4 iterations)
            if (iteration % 4 === 0) {
                promptQueue.push({ name: 'JANITOR', fn: PROMPT_RUNNERS['JANITOR'] });
                console.log(`📋 Queueing JANITOR.md (iteration ${iteration} is divisible by 4)`);
            }

            // Check for ARCHITECT.md (every 8 iterations) - skip if already in queue from continuation
            if (iteration % 8 === 0 && continuationPrompt !== 'ARCHITECT') {
                promptQueue.push({ name: 'ARCHITECT', fn: PROMPT_RUNNERS['ARCHITECT'] });
                console.log(`📋 Queueing ARCHITECT.md (iteration ${iteration} is divisible by 8)`);
            }
        } else if (LOOP_TYPE === 'bugfixer') {
            // Bugfixer loop: BUGFIXER_BUGCHECK (every 4) + BUGFIXER
            if (iteration % 4 === 0 && continuationPrompt !== 'BUGFIXER_BUGCHECK') {
                promptQueue.push({ name: 'BUGFIXER_BUGCHECK', fn: PROMPT_RUNNERS['BUGFIXER_BUGCHECK'] });
                console.log(`📋 Queueing BUGFIXER_BUGCHECK.md (iteration ${iteration} is divisible by 4)`);
            }
            promptQueue.push({ name: 'BUGFIXER', fn: PROMPT_RUNNERS['BUGFIXER'] });
        } else if (LOOP_TYPE === 'linter') {
            // Linter loop: LINTER + LINTER_SCAN (every 4) + LINTER_PRIORITIZE (every 8)
            promptQueue.push({ name: 'LINTER', fn: PROMPT_RUNNERS['LINTER'] });

            if (iteration % 4 === 0 && continuationPrompt !== 'LINTER_SCAN') {
                promptQueue.push({ name: 'LINTER_SCAN', fn: PROMPT_RUNNERS['LINTER_SCAN'] });
                console.log(`📋 Queueing LINTER_SCAN.md (iteration ${iteration} is divisible by 4)`);
            }

            if (iteration % 8 === 0 && continuationPrompt !== 'LINTER_PRIORITIZE') {
                promptQueue.push({ name: 'LINTER_PRIORITIZE', fn: PROMPT_RUNNERS['LINTER_PRIORITIZE'] });
                console.log(`📋 Queueing LINTER_PRIORITIZE.md (iteration ${iteration} is divisible by 8)`);
            }
        }

        // Execute prompts sequentially (not in parallel)
        const queueDescription = promptQueue.map(p => 
            p.isContinuation ? `${p.name}(cont)` : p.name
        ).join(' -> ');
        console.log(`📝 Execution order: ${queueDescription}`);

        for (const prompt of promptQueue) {
            if (prompt.isContinuation) {
                console.log(`\n🔄 Resuming ${prompt.name} (continuation from previous session)...`);
            } else {
                console.log(`\n▶️ Running ${prompt.name}...`);
            }
            
            const exitCode = prompt.fn();

            if (exitCode === 0) {
                // Check for .done file INSIDE the workspace
                const doneFilePath = path.join(WORKSPACE_PATH, DONE_FILE);
                if (fs.existsSync(doneFilePath)) {
                    console.log("✅ Project marked complete!");
                    process.exit(0);
                }
                // Check for loop-specific done files
                const linterDonePath = path.join(WORKSPACE_PATH, '.linter-done');
                const bugfixerDonePath = path.join(WORKSPACE_PATH, '.bugfixer-done');
                if (LOOP_TYPE === 'linter' && fs.existsSync(linterDonePath)) {
                    console.log("✅ Linter compliance complete!");
                    process.exit(0);
                }
                if (LOOP_TYPE === 'bugfixer' && fs.existsSync(bugfixerDonePath)) {
                    console.log("✅ Bug fixing complete!");
                    process.exit(0);
                }
            } else {
                console.log(`⚠️ ${prompt.name} exited with code ${exitCode}, continuing...`);
            }
        }

        console.log(`\n${new Date().toLocaleString()}: Iteration ${iteration} complete. Sleeping ${INTERVAL / 1000}s...`);

        // Save state before incrementing
        saveState(iteration);

        await new Promise(resolve => setTimeout(resolve, INTERVAL));

        // Increment iteration counter
        iteration++;
    }
}

process.on('SIGINT', () => {
    console.log("\nStopping the automation loop...");
    process.exit();
});

main();
