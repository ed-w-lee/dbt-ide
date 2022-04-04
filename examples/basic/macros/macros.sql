{% macro some_macro() %}
{{ other_macro() }}
{% endmacro %}

{% macro other_other_macro() %}
hello
{% endmacro %}