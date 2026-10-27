<?php
namespace Drupal\Component\Plugin;

abstract class PluginManagerBase implements PluginManagerInterface
{
    public function createInstance($plugin_id, array $configuration = [])
    {
    }
}
