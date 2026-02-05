const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Configuration
const INTERVAL = 0;//(process.argv[2] || 600) * 1000;
const LOOP_TYPE = process.argv[2] || 'development'; // 'development' or 'bugfixer'

// Development loop prompts
const PROMPT_PATH = path.join(__dirname, 'prompts/PROMPT.md');
const JANITOR_PATH = path.join(__dirname, 'prompts/JANITOR.md');
const ARCHITECT_PATH = path.join(__dirname, 'prompts/ARCHITECT.md');

// Bugfixer loop prompts
const BUGFIXER_PATH = path.join(__dirname, 'prompts/BUGFIXER.md');
const BUGFIXER_BUGCHECK_PATH = path.join(__dirname, 'prompts/BUGFIXER_BUGCHECK.md');

const DONE_FILE = '.done';

const WORKSPACE_PATH = path.resolve(__dirname, '../workspace');
const STATE_FILE = path.join(WORKSPACE_PATH, `.state_${LOOP_TYPE}.json`);

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

function runKilo() {
    return runKiloWithPrompt(PROMPT_PATH, 'PROMPT');
}

function runJanitor() {
    return runKiloWithPrompt(JANITOR_PATH, 'JANITOR');
}

function runArchitect() {
    return runKiloWithPrompt(ARCHITECT_PATH, 'ARCHITECT');
}

function runBugfixer() {
    return runKiloWithPrompt(BUGFIXER_PATH, 'BUGFIXER');
}

function runBugfixerBugcheck() {
    return runKiloWithPrompt(BUGFIXER_BUGCHECK_PATH, 'BUGFIXER_BUGCHECK');
}

async function main() {
    // 0. Validate loop type
    const validLoopTypes = ['development', 'bugfixer'];
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

        if (LOOP_TYPE === 'development') {
            // Development loop: PROMPT + JANITOR (every 4) + ARCHITECT (every 8)
            promptQueue.push({ name: 'PROMPT', fn: runKilo });

            // Check for JANITOR.md (every 4 iterations)
            if (iteration % 4 === 0) {
                promptQueue.push({ name: 'JANITOR', fn: runJanitor });
                console.log(`📋 Queueing JANITOR.md (iteration ${iteration} is divisible by 4)`);
            }

            // Check for ARCHITECT.md (every 8 iterations)
            if (iteration % 8 === 0) {
                promptQueue.push({ name: 'ARCHITECT', fn: runArchitect });
                console.log(`📋 Queueing ARCHITECT.md (iteration ${iteration} is divisible by 8)`);
            }
        } else if (LOOP_TYPE === 'bugfixer') {
            // Bugfixer loop: BUGFIXER_BUGCHECK (every 4) + BUGFIXER
            // Note: BUGCHECK runs FIRST on iteration 4, 8, 12, etc.
            if (iteration % 4 === 0) {
                promptQueue.push({ name: 'BUGFIXER_BUGCHECK', fn: runBugfixerBugcheck });
                console.log(`📋 Queueing BUGFIXER_BUGCHECK.md (iteration ${iteration} is divisible by 4)`);
            }
            // Always run BUGFIXER.md (runs AFTER BUGCHECK on 4th iteration)
            promptQueue.push({ name: 'BUGFIXER', fn: runBugfixer });
        }

        // Execute prompts sequentially (not in parallel)
        console.log(`📝 Execution order: ${promptQueue.map(p => p.name).join(' -> ')}`);

        for (const prompt of promptQueue) {
            console.log(`\n▶️ Running ${prompt.name}...`);
            const exitCode = prompt.fn();

            if (exitCode === 0) {
                // Check for .done file INSIDE the workspace
                const doneFilePath = path.join(WORKSPACE_PATH, DONE_FILE);
                if (fs.existsSync(doneFilePath)) {
                    console.log("✅ Project marked complete!");
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
