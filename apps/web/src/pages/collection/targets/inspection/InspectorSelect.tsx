import { SelectField, type SelectOption } from '../../../../shared/SelectField';

/** Inspector adapter: shared M2 control, optional GET-form field owned by the caller. */
export function InspectorSelect({label,value,onChange,options,name}:{
  label:string;value:string;onChange:(value:string)=>void;options:readonly SelectOption<string>[];name?:string;
}){
  return <><SelectField label={label} value={value} options={options} onChange={onChange}/>
    {name&&<input type="hidden" name={name} value={value}/>}</>;
}
