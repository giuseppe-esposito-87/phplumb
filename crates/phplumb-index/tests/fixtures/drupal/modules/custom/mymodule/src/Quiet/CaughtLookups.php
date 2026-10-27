<?php
namespace Drupal\mymodule\Quiet;

use Drupal\Component\Plugin\Exception\PluginNotFoundException;
use Drupal\Core\Block\BlockManager;
use Drupal\Core\Url;
use Symfony\Component\DependencyInjection\Exception\ServiceNotFoundException;
use Symfony\Component\Routing\Exception\RouteNotFoundException;

class CaughtLookups
{
    public function run(BlockManager $blocks)
    {
        try {
            \Drupal::service('caught.service');
        } catch (ServiceNotFoundException $e) {
        }
        try {
            $blocks->createInstance('caught_block');
        } catch (PluginNotFoundException $e) {
        }
        try {
            Url::fromRoute('caught.route');
        } catch (RouteNotFoundException $e) {
        }
    }
}
