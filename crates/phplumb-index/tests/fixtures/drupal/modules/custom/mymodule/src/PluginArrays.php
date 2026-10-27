<?php
namespace Drupal\mymodule;

use Drupal\Core\Field\BaseFieldDefinition;

class PluginArrays
{
    public function form(): array
    {
        $form['name'] = ['#type' => 'textfield', '#title' => 'Name'];
        $plain = ['type' => 'string'];
        BaseFieldDefinition::create('string')
            ->setDisplayOptions('form', ['type' => 'string_textfield'])
            ->setDisplayOptions('view', ['type' => 'string', 'weight' => 0]);
        return $form + $plain;
    }
}
