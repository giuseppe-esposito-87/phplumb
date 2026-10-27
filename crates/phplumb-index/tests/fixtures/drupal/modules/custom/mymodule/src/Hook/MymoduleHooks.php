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
}
