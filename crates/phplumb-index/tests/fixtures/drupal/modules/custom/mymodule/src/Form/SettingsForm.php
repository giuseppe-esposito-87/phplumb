<?php
namespace Drupal\mymodule\Form;

use Drupal\Core\Form\FormBase;

class SettingsForm extends FormBase
{
    public function submitForm(array &$form, $form_state): void
    {
        $this->redirect('mymodule.page');
    }
}
