<?php
namespace Drupal\basic_hooks\Hook;

use Drupal\Core\Hook\Attribute\Hook;

class BasicHooks
{
    #[Hook('cron')]
    public function cron(): void
    {
    }
}
