<?php
namespace Drupal\Core\DependencyInjection;

interface ServiceModifierInterface
{
    public function alter(ContainerBuilder $container);
}
