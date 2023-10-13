import { program } from 'commander';
import { spawnSync } from 'child_process';
import * as env from './env.js';

const COMMANDS = [
    env.command,
]

async function main() {
    const cwd = process.cwd();
    const OLAOS_HOME = process.env.OLAOS_HOME;
    if (!OLAOS_HOME) {
        throw new Error('Please set $OLAOS_HOME to the root of ola-os repo!');
    } else {
        process.chdir(OLAOS_HOME);
    }

    env.load();
    
    program.version('1.0.0').name('olaos').description('olaos workflow tools');

    for (const command of COMMANDS) {
        program.addCommand(command);
    }

    program
        .command('f <command...>')
        .allowUnknownOption()
        .action((command) => {
            process.chdir(cwd);
            const result = spawnSync(command[0], command.slice(1), { stdio: 'inherit' });
            if (result.error) {
                throw result.error;
            }
            process.exitCode = result.status || undefined;
        });

    await program.parseAsync(process.argv);
}

main().catch(err => {
    console.error('Error:', err.message || err);
    process.exitCode = 1;
});