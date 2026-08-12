<script lang="ts">
	/*
	 * Панель одной вкладки консоли: вывод сверху, строка ввода снизу.
	 * Разметка и классы — из исходного приложения.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';

	type Props = {
		activeId: number;
		/** Вывод по вкладкам — общий объект, чтобы текст переживал переключение. */
		output: Record<number, string>;
	};

	let { activeId, output = $bindable() }: Props = $props();

	let command = $state('');
	let busy = $state(false);
	let terminal = $state<HTMLElement | null>(null);

	const prompt = '$';

	$effect(() => {
		let alive = true;

		const subs = Promise.all([
			listen<string>('command-output', ({ payload }) => {
				if (alive) append(`\n${payload}`);
			}),
			listen<string>('command-error', ({ payload }) => {
				if (alive) append(`\nError: ${payload}`);
			})
		]);

		return () => {
			alive = false;
			subs.then((off) => off.forEach((unlisten) => unlisten()));
		};
	});

	function append(text: string) {
		output[activeId] = (output[activeId] ?? '') + text;
		queueMicrotask(scrollToBottom);
	}

	function scrollToBottom() {
		if (terminal) terminal.scrollTop = terminal.scrollHeight;
	}

	async function run() {
		const cmd = command.trim();
		if (!cmd) return;

		if (cmd.toLowerCase() === 'clear' || cmd.toLowerCase() === 'cls') {
			output[activeId] = '';
			command = '';
			return;
		}

		busy = true;
		try {
			await invoke('run_command', { cmd });
		} catch (error) {
			append(`\n${prompt} ${cmd}\nError: ${error}`);
		} finally {
			busy = false;
			command = '';
			scrollToBottom();
		}
	}
</script>

<div class="app_console_panel">
	<p>(DEV) ID: {activeId}</p>
	<div class="container">
		<div class="terminal" bind:this={terminal}>
			<pre>{output[activeId] ?? ''}</pre>
		</div>
		<div class="input-container">
			<span class="prompt">{prompt}</span>
			<input
				class="input"
				type="text"
				bind:value={command}
				onkeydown={(event) => {
					if (event.key === 'Enter' && !busy) run();
				}}
				placeholder="Enter command..."
				disabled={busy}
			/>
		</div>
	</div>
</div>
