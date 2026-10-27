<?php

use Drupal\Core\Extension\ModuleHandlerInterface;

class HookCalls
{
    public function run(ModuleHandlerInterface $handler): void
    {
        $handler->invokeAll('cron');
    }
}
