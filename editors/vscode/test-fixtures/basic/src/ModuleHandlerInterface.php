<?php
namespace Drupal\Core\Extension;

interface ModuleHandlerInterface
{
    public function invokeAll($hook, array $args = []);
}
