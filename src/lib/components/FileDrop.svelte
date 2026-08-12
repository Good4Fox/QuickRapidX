<script lang="ts">
	/*
	 * Зона перетаскивания файла иконки.
	 * Разметка и классы — из исходного приложения.
	 */
	const ALLOWED = ['ico', 'png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'];

	type Props = {
		/** Вызывается с принятыми файлами. */
		onfiles?: (files: File[]) => void;
	};

	let { onfiles }: Props = $props();

	let hovering = $state(false);
	let flash = $state(false);

	const accept = ALLOWED.map((ext) => `.${ext}`).join(', ');

	function isAllowed(file: File): boolean {
		const ext = file.name.split('.').pop()?.toLowerCase();
		return ext ? ALLOWED.includes(ext) : false;
	}

	function take(list: FileList | null | undefined) {
		const files = Array.from(list ?? []).filter(isAllowed);

		if (files.length === 0) {
			console.error(`допустимы только файлы: ${ALLOWED.join(', ')}`);
			return;
		}

		onfiles?.(files);

		// Короткая подсветка — подтверждение приёма, как в оригинале
		flash = true;
		setTimeout(() => (flash = false), 3000);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="drop-area"
	class:hovering
	style:background-color={flash ? 'rgba(173, 37, 59, 0.5)' : ''}
	ondragenter={(event) => {
		event.preventDefault();
		hovering = true;
	}}
	ondragover={(event) => event.preventDefault()}
	ondragleave={() => (hovering = false)}
	ondrop={(event) => {
		event.preventDefault();
		event.stopPropagation();
		hovering = false;
		take(event.dataTransfer?.files);
	}}
>
	<input type="file" {accept} multiple onchange={(event) => take(event.currentTarget.files)} />
</div>
