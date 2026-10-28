<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Small line icons, drawn on a 24-unit grid; they take the text colour. -->
<script lang="ts" module>
	export const icons = {
		sun: 'M12 4V2M12 22v-2M4 12H2M22 12h-2M5.6 5.6 4.2 4.2M19.8 19.8l-1.4-1.4M5.6 18.4l-1.4 1.4M19.8 4.2l-1.4 1.4M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8z',
		moon: 'M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.5z',
		auto: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 3v18',
		chevronDown: 'm6 9 6 6 6-6',
		chevronUp: 'm18 15-6-6-6 6',
		chevronLeft: 'm15 18-6-6 6-6',
		chevronRight: 'm9 18 6-6-6-6',
		x: 'M18 6 6 18M6 6l12 12',
		check: 'M20 6 9 17l-5-5',
		search: 'M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM21 21l-4.3-4.3',
		menu: 'M4 6h16M4 12h16M4 18h16',
		columns: 'M4 5h16v14H4zM10 5v14M15 5v14',
		sortUp: 'M12 19V5M6 11l6-6 6 6',
		sortDown: 'M12 5v14M6 13l6 6 6-6',
		sort: 'M8 9l4-4 4 4M8 15l4 4 4-4',
		plus: 'M12 5v14M5 12h14',
		trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3',
		edit: 'M4 20h4L19 9l-4-4L4 16v4zM14 6l4 4',
		link: 'M10 14a4 4 0 0 0 6 0l3-3a4 4 0 0 0-6-6l-1 1M14 10a4 4 0 0 0-6 0l-3 3a4 4 0 0 0 6 6l1-1',
		download: 'M12 4v11M7 10l5 5 5-5M5 20h14',
		bell: 'M6 16V11a6 6 0 1 1 12 0v5l2 2H4l2-2zM10 21h4',
		grip: 'M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01'
	} as const;
	export type IconName = keyof typeof icons;
</script>

<script lang="ts">
	interface Props {
		name: IconName;
		size?: number;
		label?: string;
	}

	let { name, size = 18, label }: Props = $props();
</script>

<svg
	viewBox="0 0 24 24"
	width={size}
	height={size}
	fill="none"
	stroke="currentColor"
	stroke-width="2"
	stroke-linecap="round"
	stroke-linejoin="round"
	role={label ? 'img' : undefined}
	aria-label={label}
	aria-hidden={label ? undefined : 'true'}
>
	<path d={icons[name]} />
</svg>

<style>
	svg {
		display: inline-block;
		vertical-align: -0.2em;
		flex-shrink: 0;
	}
</style>
