import { Picker, PickerItem, TextField } from '@react-spectrum/s2';

type CookieSource = {
  id: string;
  label: string;
};

type CookieSourcePanelProps = {
  items: CookieSource[];
  selectedKey: string;
  cookieFilePath: string;
  onSelectionChange: (key: string) => void;
  onCookieFilePathChange: (value: string) => void;
};

export function CookieSourcePanel(props: CookieSourcePanelProps) {
  return (
    <div className="panel-stack">
      <Picker
        label="Cookie 来源"
        selectedKey={props.selectedKey}
        onSelectionChange={(key) => props.onSelectionChange(String(key))}
        items={props.items}
      >
        {(item) => <PickerItem>{item.label}</PickerItem>}
      </Picker>
      {props.selectedKey === 'import' ? (
        <TextField
          label="cookies.txt 路径"
          value={props.cookieFilePath}
          onChange={props.onCookieFilePathChange}
          placeholder="例如 C:\\Users\\Administrator\\Downloads\\x-cookies.txt"
        />
      ) : null}
    </div>
  );
}
