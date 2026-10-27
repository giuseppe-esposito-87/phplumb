<?php
namespace Drupal\Core\Config\Entity;

class ConfigEntityStorage extends \Drupal\Core\Entity\EntityStorageBase
{
    public function configPrefix()
    {
    }
}

abstract class ConfigEntityBase implements \Drupal\Core\Entity\EntityInterface
{
    public function id()
    {
    }
}
