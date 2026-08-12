<script lang="ts">
	/* Подтверждение удаления записи. */
	import { i18n } from '$lib/state/i18n.svelte';

	type Props = {
		name: string;
		oncancel: () => void;
		onconfirm: () => void;
	};

	let { name, oncancel, onconfirm }: Props = $props();
</script>

<div class="AppStatsDelete">
	<div class="AppStatsDeletePanel">
		<div class="AppStatsDeletePanelText">
			{i18n.t.appTextUse6_3} "{name}" {i18n.t.appTextUse6_3_1}
		</div>
		<div class="AppStatsDeletePanelStatement">
			<button class="AppStatsDeletePanelStatementButton1" onclick={oncancel}>
				{i18n.t.appTextUse6_2}
			</button>
			<button class="AppStatsDeletePanelStatementButton2" onclick={onconfirm}>
				{i18n.t.appTextUse6_2_2}
			</button>
		</div>
	</div>
</div>
