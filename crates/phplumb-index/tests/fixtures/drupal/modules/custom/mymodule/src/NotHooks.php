<?php
namespace Drupal\mymodule;

use Drupal\Core\Hook\Attribute\Hook;

class NotHooks
{
    #[Hook('cron')]
    public function cron(): void
    {
    }
}
