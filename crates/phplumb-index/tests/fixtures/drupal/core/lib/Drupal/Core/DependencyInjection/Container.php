<?php
namespace Drupal\Core\DependencyInjection;

class Container implements \Symfony\Component\DependencyInjection\ContainerInterface
{
    public function get(string $id, int $invalidBehavior = 1): ?object
    {
    }
}
