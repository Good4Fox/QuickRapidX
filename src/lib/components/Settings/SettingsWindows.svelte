<script lang="ts">
	/*
	 * Раздел «Windows»: питание, восстановление системы, цвет рамки выделения,
	 * залипание клавиш и гибернация.
	 *
	 * Раскладка общая с настройками программы. Прежде здесь была своя копия тех
	 * же контейнеров — с рамкой в 2px вокруг колонки, в которой лежал один
	 * неактивный пункт, — а предпросмотр рамки выделения держался на смещениях
	 * числом (left: -448px, margin: 146px 0 0 219px).
	 */
	import { invoke } from '@tauri-apps/api/core';

	import SettingsNav from '$lib/components/Settings/SettingsNav.svelte';
	import SelectionColor from '$lib/components/Settings/SelectionColor.svelte';
	import WindowsTweaks from '$lib/components/Settings/WindowsTweaks.svelte';
	import WindowsRecovery from '$lib/components/Settings/WindowsRecovery.svelte';
	import HoldButton from '$lib/components/ui/HoldButton.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError } from '$lib/state/notifications.svelte';



	/** Прокручиваемая область: за ней следит бегунок списка разделов. */
	let scroller = $state<HTMLElement | null>(null);

	/** Часть изменений применяется только после перезахода — показываем плашку. */
	let needsRelogin = $state(false);





	/** Кнопки питания: удержание вместо подтверждения флажком. */
	const POWER = $derived([
		{ ico: 'sleep', label: i18n.t.sys_sleep, run: () => invoke('windows_sleep') },
		{ ico: 'power', label: i18n.t.sys_shutdown, run: () => invoke('windows_shutdown') },
		{ ico: 'restart', label: i18n.t.sys_restart, run: () => invoke('windows_restart') },
		{ ico: 'lock', label: i18n.t.sys_lock, run: () => invoke('windows_lock_workstation') }
	]);

	const NAV = $derived([
		{ id: 'WindowsPower', label: i18n.t.settingsWindowsText_3_1_1 },
		{ id: 'WindowsRecovery', label: i18n.t.settingsWindowsText_3_2_1 },
		{ id: 'WindowsSelection', label: i18n.t.settingsWindowsText_3_3_2 },
		{ id: 'WindowsInput', label: i18n.t.settingsWindowsText_1 }
	]);

	/** Выполняет системное действие. Отказ не должен рушить экран. */
	async function run(action: () => Promise<unknown>) {
		try {
			await action();
		} catch (e) {
			console.error('системное действие не выполнено:', e);
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}






</script>

<div class="qrx_set">
	<SettingsNav items={NAV} {scroller} />

	<div class="qrx_set_body" bind:this={scroller}>
		<div class="qrx_set_head" id="WindowsPower">{i18n.t.settingsWindowsText_3_1_1}</div>

		<!-- Быстрый доступ: безобидные действия, защищать нечего -->
		<section class="qrx_set_row qrx_set_row_stack">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.sys_quick}</span>
				<span class="qrx_set_row_note">{i18n.t.sys_quick_note}</span>
			</div>

			<div class="qrx_sys_quick">
				<button class="qrx_sys_link" onclick={() => run(() => invoke('windows_open_settings'))}>
					<span
						class="qrx_sys_link_ico qrx_sys_ico_settings"
						aria-hidden="true"
					></span>
					{i18n.t.sys_settings}
				</button>

				<button class="qrx_sys_link" onclick={() => run(() => invoke('windows_open_explorer'))}>
					<span class="qrx_sys_link_ico qrx_sys_ico_explorer" aria-hidden="true"></span>
					{i18n.t.sys_explorer}
				</button>
			</div>
		</section>

		<!-- Питание: действия необратимы, поэтому срабатывают от удержания -->
		<section class="qrx_set_row qrx_set_row_stack">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.sys_power}</span>
				<span class="qrx_set_row_note">{i18n.t.sys_power_note}</span>
			</div>

			<div class="qrx_sys_power">
				{#each POWER as action (action.label)}
					<HoldButton
						label={action.label}
						hint={i18n.t.sys_hold}
						ico="qrx_sys_ico_{action.ico}"
						onrun={() => run(action.run)}
					/>
				{/each}
			</div>
		</section>

		<div class="qrx_set_head" id="WindowsRecovery">{i18n.t.settingsWindowsText_3_2_1}</div>

		<WindowsRecovery />

		<div class="qrx_set_head" id="WindowsSelection">{i18n.t.settingsWindowsText_3_3_2}</div>

		<SelectionColor />

		<div class="qrx_set_head" id="WindowsInput">{i18n.t.settingsWindowsText_1}</div>

		<WindowsTweaks />
	</div>
</div>

{#if needsRelogin}
	<div class="app_settings_home_windows_right_panel_right_accept">
		<button
			class="app_settings_home_windows_right_panel_right_accept_button"
			onclick={() => (needsRelogin = false)}
		>
			{i18n.t.settingsWindowsText_2}
		</button>
		<button
			class="app_settings_home_windows_right_panel_right_accept_button"
			onclick={() => invoke('windows_logoff')}
		>
			{i18n.t.settingsWindowsText_2_1}
		</button>
		<button
			class="app_settings_home_windows_right_panel_right_accept_button"
			onclick={() => invoke('windows_restart')}
		>
			{i18n.t.settingsWindowsText_2_2}
		</button>
	</div>
{/if}
