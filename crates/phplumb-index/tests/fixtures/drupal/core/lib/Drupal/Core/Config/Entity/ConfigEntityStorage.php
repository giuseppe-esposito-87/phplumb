<?php
namespace Drupal\Core\Config\Entity;

class ConfigEntityStorage extends \Drupal\Core\Entity\EntityStorageBase
{
    public function /*@target:configPrefix*/configPrefix()
    {
    }
}

abstract class ConfigEntityBase implements \Drupal\Core\Entity\EntityInterface
{
    public function id()
    {
    }
}
