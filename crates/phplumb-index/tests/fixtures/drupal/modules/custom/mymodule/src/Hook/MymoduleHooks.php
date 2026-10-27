<?php
namespace Drupal\mymodule\Hook;

use Drupal\Core\Hook\Attribute\Hook;

#[Hook('cron', method: 'tick')]
class MymoduleHooks
{
    #[Hook(/*@cursor:cron_impl*/'cron')]
    public function /*@cursor:cron_impl*/cron(): void
    {
    }

    public function /*@cursor:cron_impl*/tick(): void
    {
    }

    #[Hook('cron', module: 'gadget')]
    public function gadgetCron(): void
    {
    }

    #[Hook('thing_presave')]
    public function /*@cursor:presave*/thingPresave(): void
    {
    }

    #[Hook('entity_presave')]
    public function /*@cursor:entity_presave*/entityPresave(): void
    {
    }

    #[Hook('setting_presave')]
    public function /*@cursor:setting_presave*/settingPresave(): void
    {
    }

    #[Hook('thing_view')]
    public function /*@cursor:thing_view*/thingView(): void
    {
    }

    #[Hook('thing_view_alter')]
    public function /*@cursor:thing_view_alter*/thingViewAlter(): void
    {
    }

    #[Hook('report_presave')]
    public function /*@target:report*//*@cursor:report*/reportPresave(): void
    {
    }
}
