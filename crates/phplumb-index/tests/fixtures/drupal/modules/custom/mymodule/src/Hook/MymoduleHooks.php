<?php
namespace Drupal\mymodule\Hook;

use Drupal\Core\Hook\Attribute\Hook;

#[Hook('cron', method: 'tick')]
class MymoduleHooks
{
    #[Hook('cron')]
    public function cron(): void
    {
    }

    public function tick(): void
    {
    }

    #[Hook('cron', module: 'gadget')]
    public function gadgetCron(): void
    {
    }
}
