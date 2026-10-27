<?php
namespace Drupal\Core\DependencyInjection;

interface ServiceProviderInterface
{
    public function register(ContainerBuilder $container);
}
