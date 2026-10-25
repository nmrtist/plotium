// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
	site: 'https://plotium.nmrtist.space',
	integrations: [
		starlight({
			title: 'Plotium',
			description: 'User manual for Plotium, a desktop application for scientific data analysis and figure preparation.',
			logo: {
				light: './src/assets/logo-light.svg',
				dark: './src/assets/logo-dark.svg',
			},
			defaultLocale: 'root',
			locales: {
				root: { label: 'English', lang: 'en' },
				'zh-cn': { label: '简体中文', lang: 'zh-CN' },
			},
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/nmrtist/plotium' }],
			sidebar: [
				{
					label: 'Getting Started',
					translations: { 'zh-CN': '快速上手' },
					items: [
						{ slug: 'getting-started/installation' },
						{ slug: 'getting-started/quick-tour' },
						{ slug: 'getting-started/first-figure' },
					],
				},
				{
					label: 'AI Assistant',
					translations: { 'zh-CN': 'AI 助手' },
					items: [
						{ slug: 'ai/connect' },
						{ slug: 'ai/working-with-ai' },
						{ slug: 'ai/control-and-review' },
					],
				},
				{
					label: 'Data',
					translations: { 'zh-CN': '准备数据' },
					items: [
						{ slug: 'guides/importing-data' },
						{ slug: 'guides/supported-formats' },
						{ slug: 'guides/tables' },
						{ slug: 'guides/organizing-data' },
					],
				},
				{
					label: 'Processing',
					translations: { 'zh-CN': '处理' },
					items: [
						{ slug: 'guides/processing' },
						{ slug: 'guides/templates' },
					],
				},
				{
					label: 'Analysis',
					translations: { 'zh-CN': '分析' },
					items: [
						{ slug: 'guides/choosing-a-tool' },
						{ slug: 'guides/peaks-and-regions' },
						{ slug: 'guides/curve-fitting' },
						{ slug: 'guides/custom-models' },
						{ slug: 'guides/statistics' },
						{ slug: 'guides/spectrum-tools' },
					],
				},
				{
					label: 'By data type',
					translations: { 'zh-CN': '按数据类型' },
					items: [
						{
							label: 'NMR',
							items: [
								{ slug: 'guides/craft' },
								{ slug: 'guides/pseudo-2d' },
								{ slug: 'guides/dosy' },
								{ slug: 'guides/2d-integration' },
								{ slug: 'guides/symmetry-review' },
							],
						},
						{ slug: 'guides/mass-spectrometry' },
						{ slug: 'guides/xps' },
						{ slug: 'guides/electrophysiology' },
					],
				},
				{
					label: 'Figures',
					translations: { 'zh-CN': '图形' },
					items: [
						{ slug: 'guides/plots-and-charts' },
						{ slug: 'guides/axes' },
						{ slug: 'guides/legends-and-lines' },
						{ slug: 'guides/themes-and-styles' },
						{ slug: 'guides/multi-panel-figures' },
						{ slug: 'guides/images' },
						{ slug: 'guides/annotations' },
						{ slug: 'guides/contour-levels' },
						{ slug: 'guides/heatmap-range' },
						{ slug: 'guides/exporting' },
						{ slug: 'guides/present-mode' },
					],
				},
				{
					label: 'Batch and Scripting',
					translations: { 'zh-CN': '批量与脚本' },
					items: [
						{ slug: 'guides/automation' },
						{ slug: 'reference/cli' },
					],
				},
				{
					label: 'Reference',
					translations: { 'zh-CN': '参考' },
					items: [
						{ slug: 'reference/ui-overview' },
						{ slug: 'reference/ribbon' },
						{ slug: 'reference/command-palette' },
						{ slug: 'reference/shortcuts' },
						{ slug: 'reference/preferences' },
						{ slug: 'reference/file-formats' },
					],
				},
				{
					label: 'Setting IDs and MCP',
					translations: { 'zh-CN': '设置 ID 与 MCP' },
					items: [
						{ slug: 'reference/settings-ids' },
						{
							label: 'MCP',
							items: [
								{ slug: 'reference/mcp' },
								{ slug: 'reference/mcp/discovery' },
								{ slug: 'reference/mcp/tables' },
								{ slug: 'reference/mcp/plots' },
								{ slug: 'reference/mcp/styling' },
								{ slug: 'reference/mcp/layout' },
								{ slug: 'reference/mcp/review' },
								{ slug: 'reference/mcp/save-and-export' },
								{ slug: 'reference/mcp/scientific' },
							],
						},
					],
				},
				{
					label: 'Help',
					translations: { 'zh-CN': '帮助' },
					items: [
						{ slug: 'reference/troubleshooting' },
						{ slug: 'reference/reporting-problems' },
						{ slug: 'reference/updates' },
					],
				},
			],
		}),
	],
});
