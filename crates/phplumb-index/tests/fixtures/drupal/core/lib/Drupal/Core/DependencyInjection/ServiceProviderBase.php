<?php
namespace Drupal\Core\DependencyInjection;

abstract class ServiceProviderBase implements ServiceProviderInterface, ServiceModifierInterface
{
    public function register(ContainerBuilder $container)
    {
    }

    public function alter(ContainerBuilder $container)
    {
    }
}
