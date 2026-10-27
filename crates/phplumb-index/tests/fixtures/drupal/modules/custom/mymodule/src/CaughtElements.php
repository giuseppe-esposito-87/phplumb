<?php
namespace Drupal\mymodule;

use Drupal\Component\Plugin\Exception\PluginNotFoundException;

class CaughtElements
{
    public function build(): array
    {
        try {
            \Drupal::service('plugin.manager.block')->createInstance('caught_block');
        } catch (PluginNotFoundException $e) {
        }
        return ['#type' => 'nowhere_caught_element'];
    }
}
